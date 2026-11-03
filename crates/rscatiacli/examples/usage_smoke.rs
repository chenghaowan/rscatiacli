//! 0.1.0 实际使用冒烟测试：在**新建的临时 Part 文档**上走全链路
//! （阶段 1/3/5 的写入路径 + 读回验证），结束时不保存直接关闭，不留痕。
//!
//! 需要已启动的 CATIA 会话（守卫不自动拉起）：
//! ```text
//! cargo run -p rscatiacli --release --example usage_smoke
//! ```

use std::time::Duration;

use rscatiacli::{
    objects::{Inertia, Pad, Parameter, RealParam, Sketch},
    with_application, ComObject, FromComObj,
};

fn main() {
    let result = with_application(
        Duration::from_secs(300),
        |_msg| {},
        |app, log| {
            // ── 1. 新建临时 Part 文档（不落盘） ──
            let doc = app.documents()?.add("Part")?;
            let part = doc.part_document().part()?;
            log("Part 文档已新建");

            // ── 2. 阶段 3：主 Body 内草图 + 50x30 矩形轮廓 ──
            let body = part.main_body()?;
            part.set_in_work_object(&body)?;
            let plane = part.origin_elements()?.plane_xy()?;
            let sketch: Sketch = body.sketches()?.add(&plane)?;
            let f2d = sketch.open_edition()?;
            let _lines = [
                f2d.create_line(0.0, 0.0, 50.0, 0.0)?,
                f2d.create_line(50.0, 0.0, 50.0, 30.0)?,
                f2d.create_line(50.0, 30.0, 0.0, 30.0)?,
                f2d.create_line(0.0, 30.0, 0.0, 0.0)?,
            ];
            sketch.close_edition()?;
            part.update()?;
            log("草图 50x30 矩形已画（4 条线）");

            // ── 3. 阶段 1：Pad 拉伸（写入链核心验证） ──
            let sf = part.shape_factory()?;
            let pad: Pad = sf.add_new_pad(&sketch, 10.0)?;
            part.update()?;
            let pad_name = pad.name()?;
            log(&format!("Pad 拉伸 10mm 成功: {pad_name}"));

            // ── 4. 阶段 5：类型化参数 + 公式驱动（改参数 → 模型联动） ──
            let params = part.parameters()?;
            let thickness: RealParam = params.create_real("Smoke.Thickness", 20.0)?;
            part.update()?;
            let v = thickness.value()?;
            assert!((v - 20.0).abs() < 1e-9, "参数读回 {v} != 20.0");
            let rels = part.relations()?;
            let output = <Parameter as FromComObj>::from_com(thickness.com().clone());
            let _f = rels.create_formula("Smoke.Formula.1", "", &output, "5 * 4")?;
            part.update()?;
            let v2 = thickness.value()?;
            assert!((v2 - 20.0).abs() < 1e-9, "公式后参数 {v2} != 20.0");
            log(&format!("参数+公式生效（值 {v2}）"));

            // ── 5. 阶段 2 读回：Inertia 质量属性 + 密度 ──
            // 真机实测：全新 Part 文档上 Part.Analyze 成员缺失，走
            // GetTechnologicalObject("Inertia")（wcc/find_near_parts 同通道）。
            let density = part.density()?;
            let product = doc.product_document().product()?;
            let raw = product.get_technological_object("Inertia")?;
            let inertia = Inertia::from_com(raw.com().clone());
            let ss = app.system_service()?;
            let mass = inertia.mass()?;
            let cog = inertia.get_cog_position(&ss)?;
            log(&format!(
                "Inertia.mass = {mass:?} kg, cog = {cog:?}, density = {density:?}"
            ));

            // ── 6. 关闭不保存（不留痕） ──
            doc.close()?;
            log("临时文档已关闭（不保存）");

            Ok(())
        },
    );
    match result {
        Ok(_) => println!("usage smoke: OK —— 0.1.0 写入链真机通过"),
        Err(e) => {
            eprintln!("usage smoke FAILED: {e}");
            std::process::exit(1);
        }
    }
}
