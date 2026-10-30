//! CATIA 会话：专用 STA 线程、ROT 连接、多实例守卫。
//!
//! 设计要点（与 ltl-manage 既有 Python 纪律一一对应）：
//!
//! 1. **专用 STA 线程**：CATIA 是 STA 自动化服务器，所有 COM 调用必须在同一条
//!    `CoInitializeEx(COINIT_APARTMENTTHREADED)` 的线程上执行；外部通过 channel 投递。
//!    调用方带超时等待 —— COM 调用无法取消，卡住的线程只能废弃，因此超时后丢弃该
//!    worker，下次调用自动重建。
//! 2. **只用 ROT 连接**：`GetActiveObject`，绝不 `CoCreateInstance`/按名字 Dispatch。
//!    后者在 ROT 没有 CATIA 时会静默拉起一个无窗口后台实例，下一次调用就会触发
//!    “检测到多个 CATIA 实例”。
//! 3. **每次操作重新 attach，结束即丢弃**：不跨操作缓存 Application/Document，
//!    这样 CATIA 重启、文档被关都不会留下悬空引用。
//! 4. **多实例守卫 + 空会话自动退出**：与 Python 侧 get_catia_com 行为一致。

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::core::{IUnknown, Interface, PCWSTR};
use windows::Win32::System::Com::{
    CLSIDFromProgID, CoInitializeEx, CoUninitialize, IDispatch, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Ole::GetActiveObject;

use crate::collections::ComCollection;
use crate::dispatch::{ComObj, ComObject};
use crate::error::{ComError, ComResult};
use crate::objects::Application;

/// 进度回调：把日志交给调用方（CLI 打印 / App 内 emit 事件）。
pub type Progress<'a> = &'a dyn Fn(&str);

const PROGID_CATIA: &str = "CATIA.Application";
const CNEXT_EXE: &str = "CNEXT.exe";

// ── STA 工作线程 ─────────────────────────────────────────────────────

type Job = Box<dyn FnOnce() + Send>;

struct ComWorker {
    tx: Sender<Job>,
}

impl ComWorker {
    fn spawn() -> ComResult<Self> {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("catia-com-sta".into())
            .spawn(move || {
                // SAFETY: 线程入口处初始化 COM apartment；退出时反初始化。
                // S_FALSE(已初始化) 与 RPC_E_CHANGED_MODE 都不致命，继续尝试调用。
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();
                }
                while let Ok(job) = rx.recv() {
                    job();
                }
                unsafe {
                    CoUninitialize();
                }
            })
            .map_err(|e| ComError::runtime(format!("创建 CATIA COM 线程失败: {e}")))?;
        Ok(Self { tx })
    }

    fn submit<T, F>(&self, timeout: Duration, f: F) -> Result<T, ComError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let (done_tx, done_rx) = mpsc::channel();
        self.tx
            .send(Box::new(move || {
                let _ = done_tx.send(f());
            }))
            .map_err(|_| ComError::runtime("CATIA COM 工作线程已退出，请重试"))?;

        match done_rx.recv_timeout(timeout) {
            Ok(v) => Ok(v),
            Err(RecvTimeoutError::Timeout) => Err(ComError::runtime(
                "CATIA 未响应：操作超过等待时限。请检查 CATIA 是否弹出对话框或卡死，处理后重试",
            )),
            Err(RecvTimeoutError::Disconnected) => {
                Err(ComError::runtime("CATIA COM 工作线程异常退出，请重试"))
            }
        }
    }
}

static WORKER: Mutex<Option<ComWorker>> = Mutex::new(None);

/// 在 STA 线程上执行一段 COM 操作并等待结果。
///
/// 超时后该 worker 被丢弃（线程可能仍卡在 COM 调用里，无法回收），
/// 下一次调用会新建一条 STA 线程与新的 apartment。
pub fn run_on_sta<T, F>(timeout: Duration, f: F) -> Result<T, ComError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let worker_tx = {
        let mut slot = WORKER.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            *slot = Some(ComWorker::spawn()?);
        }
        slot.as_ref().expect("worker just created").tx.clone()
    };
    let worker = ComWorker { tx: worker_tx };
    let result = worker.submit(timeout, f);

    if let Err(e) = &result {
        // 超时/断连：当前 worker 状态不可信，丢弃以便下次重建。
        if e.message.contains("未响应") || e.message.contains("异常退出") {
            let mut slot = WORKER.lock().unwrap_or_else(|e| e.into_inner());
            *slot = None;
        }
    }
    result
}

/// 在 STA 线程上连接 CATIA 并执行一段业务逻辑，返回业务结果。
///
/// `progress` 会同时用于连接阶段（空会话自动退出提示）和业务阶段。
pub fn with_application<T, P, F>(timeout: Duration, progress: P, f: F) -> ComResult<T>
where
    T: Send + 'static,
    P: Fn(&str) + Send + 'static,
    F: FnOnce(&Application, &dyn Fn(&str)) -> ComResult<T> + Send + 'static,
{
    match run_on_sta(timeout, move || -> ComResult<T> {
        let log = |msg: &str| progress(msg);
        let app = attach_with_guard(&log)?;
        f(&app, &log)
    }) {
        Ok(inner) => inner,
        Err(e) => Err(e),
    }
}

// ── 连接与守卫 ───────────────────────────────────────────────────────

/// 当前运行的 CATIA 主进程（CNEXT.exe）数量。
pub fn cnext_count() -> usize {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .filter(|p| p.name().to_string_lossy().eq_ignore_ascii_case(CNEXT_EXE))
        .count()
}

pub fn catia_is_running() -> bool {
    cnext_count() > 0
}

/// 进程诊断：返回（sysinfo 进程总数, 名字含 `needle` 的进程名列表）。
pub fn process_names_containing(needle: &str) -> (usize, Vec<String>) {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let all: Vec<String> = sys
        .processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_string())
        .collect();
    let total = all.len();
    let needle = needle.to_ascii_lowercase();
    let hits = all
        .into_iter()
        .filter(|n| n.to_ascii_lowercase().contains(&needle))
        .collect();
    (total, hits)
}

/// ROT 连接正在运行的 CATIA。`wait` 内轮询，等 CATIA 启动完成注册 COM。
fn attach_rot(wait: Duration) -> ComResult<ComObj> {
    let progid: Vec<u16> = PROGID_CATIA
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: progid 在本帧存活且以 NUL 结尾。
    let clsid = unsafe { CLSIDFromProgID(PCWSTR(progid.as_ptr())) }.map_err(|e| {
        ComError::new(
            e.code().0,
            format!("无法解析 CATIA ProgID: {}", e.message()),
        )
    })?;

    let deadline = Instant::now() + wait;
    loop {
        let mut punk: Option<IUnknown> = None;
        // SAFETY: clsid 有效；punk 由 COM 写入并持有引用。
        let hit = unsafe { GetActiveObject(&clsid, None, &mut punk) };
        if hit.is_ok() {
            if let Some(unk) = punk {
                let disp: IDispatch = unk.cast().map_err(|e| {
                    ComError::new(
                        e.code().0,
                        format!("CATIA 对象不是 IDispatch: {}", e.message()),
                    )
                })?;
                return Ok(ComObj(disp));
            }
        }
        if Instant::now() >= deadline {
            return Err(ComError::runtime(format!(
                "CATIA 进程在运行但 COM 接口迟迟未就绪（等了 {} 秒）：可能正在启动，请等 CATIA 主窗口出现后重试",
                wait.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn doc_names(app: &Application) -> Vec<String> {
    let Ok(docs) = app.documents() else {
        return Vec::new();
    };
    let Ok(items) = docs.items() else {
        return Vec::new();
    };
    items
        .iter()
        .map(|doc| doc.name().unwrap_or_else(|_| "?".to_string()))
        .collect()
}

/// 多实例守卫：本机有多个 CNEXT 且连到的实例有文档时，无法确定哪个是用户正在用的，
/// 明确报错而不是把操作静默写进错误的 CATIA（实测 PA6 曾挂进无关 Part1）。
fn guard_multi_instance(app: &Application) -> ComResult<()> {
    let n = cnext_count();
    if n <= 1 {
        return Ok(());
    }
    let names = doc_names(app);
    Err(ComError::runtime(format!(
        "检测到本机有 {n} 个 CATIA 实例正在运行, 自动化无法确定你正在操作的是哪一个\
         (本次连接到的实例打开着: {})。\
         为避免把操作写进错误的 CATIA 窗口, 请关闭多余的 CATIA 会话(只保留你正在用的那个)后重试。\
         若你只看到一个 CATIA 窗口: 多出的多半是自动化残留的无窗口后台实例, \
         在任务管理器里结束没有主窗口的 CNEXT.exe 即可。",
        if names.is_empty() {
            "无文档".to_string()
        } else {
            names.join(", ")
        }
    )))
}

fn document_count(app: &Application) -> Option<i32> {
    app.documents().ok()?.count().ok()
}

/// 连接正在运行的 CATIA（绝不自动拉起），并执行多实例守卫。
///
/// 行为对齐 Python 侧 `get_catia_com()`：
/// - 进程不在 → 明确报错；
/// - ROT 未就绪 → 轮询最多 20s；
/// - 连到的实例没有文档而本机还有其他 CNEXT → 判定为后台残留空会话，
///   Quit 后重连（最多 4 轮）。
pub fn attach_with_guard(progress: Progress<'_>) -> ComResult<Application> {
    if !catia_is_running() {
        return Err(ComError::runtime(
            "CATIA 未运行: 请先启动 CATIA 再使用工具(工具不会自动拉起 CATIA —— \
             自动拉起产生的是无窗口的后台实例, 用户看不见, \
             会让后续所有工具报「检测到多个 CATIA 实例」)",
        ));
    }

    let mut app = Application::new(attach_rot(Duration::from_secs(20))?);
    let Some(mut n_docs) = document_count(&app) else {
        // 文档数读不出（状态未知）：不做多余动作，交由调用方报错。
        return Ok(app);
    };

    for _ in 0..4 {
        if n_docs > 0 || cnext_count() <= 1 {
            guard_multi_instance(&app)?;
            return Ok(app);
        }

        progress(
            "连接到的 CATIA 实例没有打开任何文档, 而本机还有其他 CATIA 进程: \
             判定为后台残留空会话, 自动关闭后重连",
        );
        if app.com().call("Quit", &[]).is_err() {
            return Ok(app);
        }
        std::thread::sleep(Duration::from_millis(500));

        app = Application::new(attach_rot(Duration::from_secs(10)).map_err(|_| {
            ComError::runtime("已自动关闭后台空 CATIA 会话, 但重连 CATIA 失败, 请稍后重试")
        })?);
        let Some(n) = document_count(&app) else {
            return Ok(app);
        };
        n_docs = n;
    }

    guard_multi_instance(&app)?;
    Ok(app)
}

// ── 只读探针 / 诊断 ──────────────────────────────────────────────────

/// 连接状态探测。字段与 Python 版 `action_status()`
/// （ltl-manage resources/sidecars/link_fixer/catia_link_fixer_bridge.py）保持一致，
/// 前端无需改动。**必须在 STA 线程上执行；不抛错，异常写进 error 字段。**
fn status_inner() -> serde_json::Value {
    let processes = cnext_count();
    let mut out = serde_json::json!({
        "connected": false,
        "com_available": true,
        "handler_available": true,
        "catia_running": processes > 0,
        "documents": [],
        "product_count": 0,
        "doc_count": 0,
        "catia_processes": processes,
    });

    if processes == 0 {
        return out;
    }

    // 状态探测与 Python 版一致：只连接、不加多实例守卫（由前端提示用户）。
    let app = match attach_rot(Duration::from_secs(20)) {
        Ok(app) => Application::new(app),
        Err(e) => {
            out["error"] = serde_json::Value::String(e.to_string());
            return out;
        }
    };

    let mut documents = Vec::new();
    let mut product_count = 0i64;
    let mut doc_count = 0i64;

    if let Ok(docs) = app.documents() {
        if let Ok(count) = docs.count() {
            doc_count = i64::from(count);
            for i in 1..=count {
                let Ok(doc) = docs.item(i) else { continue };

                let name = doc.name().unwrap_or_else(|_| "?".to_string());
                let path = doc.full_name().unwrap_or_default();
                let readonly = doc.is_read_only().unwrap_or(false);
                let is_product = name.to_ascii_lowercase().ends_with(".catproduct");
                if is_product {
                    product_count += 1;
                }
                documents.push(serde_json::json!({
                    "name": name,
                    "path": path,
                    "readonly": readonly,
                    "is_product": is_product,
                }));
            }
        }
    }

    out["connected"] = serde_json::Value::Bool(true);
    out["doc_count"] = serde_json::Value::from(doc_count);
    out["product_count"] = serde_json::Value::from(product_count);
    out["documents"] = serde_json::Value::Array(documents);
    out
}

/// 状态探测入口（供 Tauri 命令 / CLI 调用，内部投递到 STA 线程）。
pub fn status() -> serde_json::Value {
    match run_on_sta(Duration::from_secs(60), status_inner) {
        Ok(v) => v,
        Err(e) => serde_json::json!({
            "connected": false,
            "com_available": true,
            "handler_available": true,
            "catia_running": cnext_count() > 0,
            "documents": [],
            "product_count": 0,
            "doc_count": 0,
            "catia_processes": cnext_count(),
            "error": e.to_string(),
        }),
    }
}

/// 连接 CATIA 并返回活动文档信息，**不修改任何东西**。
/// 内部投递到 STA 线程，可直接从任意线程调用。
pub fn probe() -> ComResult<String> {
    match run_on_sta(Duration::from_secs(60), probe_inner) {
        Ok(inner) => inner,
        Err(e) => Err(e),
    }
}

/// 探针主体。**必须在 STA 线程上执行。**
fn probe_inner() -> ComResult<String> {
    let app = attach_with_guard(&|msg| eprintln!("{msg}"))?;
    let doc = app
        .active_document()
        .map_err(|_| ComError::runtime("CATIA 中没有打开的文档"))?;
    let name = doc.name().unwrap_or_else(|_| "?".to_string());
    let mut out = format!("活动文档: {name}");

    match doc.part() {
        Ok(part) => {
            let n = part.bodies().and_then(|b| b.count()).unwrap_or(-1);
            out.push_str(&format!("\n类型: 零件(.CATPart)\nBody 数量: {n}"));
        }
        Err(_) => out.push_str("\n类型: 非零件(装配/工程图等) —— colorize 会拒绝执行"),
    }
    Ok(out)
}

/// 诊断：把每个 Body 的名字原样写回（验证属性赋值 DISPATCH_PROPERTYPUT 通道，
/// 不产生任何可见变化）。内部投递到 STA 线程，可直接从任意线程调用。
pub fn put_selftest() -> ComResult<String> {
    match run_on_sta(Duration::from_secs(60), put_selftest_inner) {
        Ok(inner) => inner,
        Err(e) => Err(e),
    }
}

/// 自检主体。**必须在 STA 线程上执行。**
fn put_selftest_inner() -> ComResult<String> {
    let app = attach_with_guard(&|msg| eprintln!("{msg}"))?;
    let doc = app.active_document()?;
    let part = doc.part()?;
    let bodies = part.bodies()?;
    let n = bodies.count()?;

    let mut out = Vec::new();
    for i in 1..=n {
        let body = bodies.item(i)?;
        let name = body.name()?;
        body.set_name(&name)?;
        out.push(format!("Body[{i}] {name} -> 写回 OK"));
    }
    Ok(out.join("\n"))
}
