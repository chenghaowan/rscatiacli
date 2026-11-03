//! 空间分析补充族（pycatia `space_analyses_interfaces`，阶段 8）。
//!
//! 干涉检查（`Clashes`/`Clash`/`ClashResult(s)`/`Conflict(s)`）、断面
//! （`Sections`/`Section`）、惯量集合（`Inertias`，元素为既有
//! [`Inertia`](super::inertia::Inertia)）。既有 `SPAWorkbench`/`Measurable`/
//! `Distances`/`Distance`/`Inertia` 在本目录其它文件。
//!
//! 获取路径：`SPAWorkbench::clashes()` / `sections()`（CAA 属性 Clashes/Sections）；
//! `Inertias` 经 `Product::GetTechnologicalObject("Inertias")`（走 com()）。
//! `Section`/`Clash` 的 `Type` 属性是 Rust 保留字，方法名带前缀。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::groups::Group;
use super::inertia::Inertia;
use super::system_service::SystemService;

com_wrapper!(Clashes);
com_wrapper!(Clash);
com_wrapper!(ClashResults);
com_wrapper!(ClashResult);
com_wrapper!(Conflicts);
com_wrapper!(Conflict);
com_wrapper!(Inertias);
com_wrapper!(Sections);
com_wrapper!(Section);

impl ComCollection for Clashes {
    type Item = Clash;
}

impl ComCollection for ClashResults {
    type Item = ClashResult;
}

impl ComCollection for Conflicts {
    type Item = Conflict;
}

impl ComCollection for Inertias {
    type Item = Inertia;
}

impl ComCollection for Sections {
    type Item = Section;
}

impl Clashes {
    /// 新建干涉检查（`Add`）。
    pub fn add(&self) -> ComResult<Clash> {
        Ok(Clash::new(self.com().call("Add", &[])?.as_obj()?))
    }

    /// 从当前选择集新建（`AddFromSel`）。
    pub fn add_from_sel(&self) -> ComResult<Clash> {
        Ok(Clash::new(self.com().call("AddFromSel", &[])?.as_obj()?))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl Clash {
    prop_pair!(clearance, set_clearance, "Clearance", f64);
    prop_pair!(
        computation_type,
        set_computation_type,
        "ComputationType",
        i32
    );
    prop_pair!(
        interference_type,
        set_interference_type,
        "InterferenceType",
        i32
    );

    /// 第一组（`FirstGroup`）。
    pub fn first_group(&self) -> ComResult<Group> {
        Ok(Group::new(self.com().get("FirstGroup")?.as_obj()?))
    }

    /// 设置第一组（`FirstGroup`）。
    pub fn set_first_group(&self, group: &Group) -> ComResult<()> {
        self.com().put("FirstGroup", Arg::Obj(group.com().clone()))
    }

    /// 第二组（`SecondGroup`）。
    pub fn second_group(&self) -> ComResult<Group> {
        Ok(Group::new(self.com().get("SecondGroup")?.as_obj()?))
    }

    /// 设置第二组（`SecondGroup`）。
    pub fn set_second_group(&self, group: &Group) -> ComResult<()> {
        self.com().put("SecondGroup", Arg::Obj(group.com().clone()))
    }

    /// 冲突结果集合（`Conflicts`；compute 后读）。
    pub fn conflicts(&self) -> ComResult<Conflicts> {
        Ok(Conflicts::new(self.com().get("Conflicts")?.as_obj()?))
    }

    /// 执行干涉计算（`Compute`）。
    pub fn compute(&self) -> ComResult<()> {
        self.com().call("Compute", &[]).map(|_| ())
    }

    /// 导出结果（`Export(type, path)`；type 为 CatClashExportType）。
    pub fn export(&self, export_type: i32, path: &str) -> ComResult<()> {
        self.com()
            .call(
                "Export",
                &[Arg::I32(export_type), Arg::Str(path.to_string())],
            )
            .map(|_| ())
    }
}

impl ClashResults {
    /// 从 XML 导入（`AddFromXML(path, type)`）。
    pub fn add_from_xml(&self, path: &str, clash_type: i32) -> ComResult<ClashResult> {
        Ok(ClashResult::new(
            self.com()
                .call(
                    "AddFromXML",
                    &[Arg::Str(path.to_string()), Arg::I32(clash_type)],
                )?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl ClashResult {
    /// 冲突列表（`Conflicts`）。
    pub fn conflicts(&self) -> ComResult<Conflicts> {
        Ok(Conflicts::new(self.com().get("Conflicts")?.as_obj()?))
    }

    /// 导出（`Export(type, path)`）。
    pub fn export(&self, export_type: i32, path: &str) -> ComResult<()> {
        self.com()
            .call(
                "Export",
                &[Arg::I32(export_type), Arg::Str(path.to_string())],
            )
            .map(|_| ())
    }
}

impl Conflict {
    prop_pair!(comment, set_comment, "Comment", str);
    prop_pair!(status, set_status, "Status", i32);

    /// 冲突类型（`Type`；Rust 保留字故带 conflict 前缀）。
    pub fn conflict_type(&self) -> ComResult<i32> {
        self.com().get("Type")?.as_i32()
    }

    /// 比较信息（`ComparisonInfo`，原始对象）。
    pub fn comparison_info(&self) -> ComResult<ComObj> {
        self.com().get("ComparisonInfo")?.as_obj()
    }

    /// 产品 1（`FirstProduct`）。
    pub fn first_product(&self) -> ComResult<ComObj> {
        self.com().get("FirstProduct")?.as_obj()
    }

    /// 产品 2（`SecondProduct`）。
    pub fn second_product(&self) -> ComResult<ComObj> {
        self.com().get("SecondProduct")?.as_obj()
    }

    /// 冲突值（`Value`，原始对象）。
    pub fn value(&self) -> ComResult<ComObj> {
        self.com().get("Value")?.as_obj()
    }

    /// 干涉点 1 坐标（`GetFirstPointCoordinates`，VBA 中转；3 分量）。
    pub fn get_first_point_coordinates(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        let v = ss.numeric_out_param(self.com(), "GetFirstPointCoordinates")?;
        Ok([
            v.first().copied().unwrap_or(0.0),
            v.get(1).copied().unwrap_or(0.0),
            v.get(2).copied().unwrap_or(0.0),
        ])
    }

    /// 干涉点 2 坐标（`GetSecondPointCoordinates`，VBA 中转；3 分量）。
    pub fn get_second_point_coordinates(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        let v = ss.numeric_out_param(self.com(), "GetSecondPointCoordinates")?;
        Ok([
            v.first().copied().unwrap_or(0.0),
            v.get(1).copied().unwrap_or(0.0),
            v.get(2).copied().unwrap_or(0.0),
        ])
    }
}

impl Inertias {
    /// 对象加入惯量集合（`Add(object)` -> Inertia）。
    pub fn add(&self, object: &impl ComObject) -> ComResult<Inertia> {
        Ok(Inertia::new(
            self.com()
                .call("Add", &[Arg::Obj(object.com().clone())])?
                .as_obj()?,
        ))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl Sections {
    /// 新建断面（`Add`）。
    pub fn add(&self) -> ComResult<Section> {
        Ok(Section::new(self.com().call("Add", &[])?.as_obj()?))
    }

    /// 从当前选择集新建（`AddFromSel`）。
    pub fn add_from_sel(&self) -> ComResult<Section> {
        Ok(Section::new(self.com().call("AddFromSel", &[])?.as_obj()?))
    }

    /// 删除（`Remove(index)`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl Section {
    prop_pair!(behavior, set_behavior, "Behavior", i32);
    prop_pair!(cut_mode, set_cut_mode, "CutMode", i32);
    prop_pair!(thickness, set_thickness, "Thickness", f64);
    prop_pair!(width, set_width, "Width", f64);
    prop_pair!(height, set_height, "Height", f64);

    /// 断面类型（`Type`；Rust 保留字故带 section 前缀）。
    pub fn section_type(&self) -> ComResult<i32> {
        self.com().get("Type")?.as_i32()
    }

    /// 设置断面类型（`Type`）。
    pub fn set_section_type(&self, value: i32) -> ComResult<()> {
        self.com().put("Type", Arg::I32(value))
    }

    /// 关联组（`Group`）。
    pub fn group(&self) -> ComResult<Group> {
        Ok(Group::new(self.com().get("Group")?.as_obj()?))
    }

    /// 设置关联组（`Group`）。
    pub fn set_group(&self, group: &Group) -> ComResult<()> {
        self.com().put("Group", Arg::Obj(group.com().clone()))
    }

    /// 断面是否为空（`IsEmpty`）。
    pub fn is_empty(&self) -> ComResult<bool> {
        self.com().call("IsEmpty", &[])?.as_bool()
    }

    /// 导出（`Export`）。
    pub fn export(&self) -> ComResult<()> {
        self.com().call("Export", &[]).map(|_| ())
    }

    /// 断面位置分量（`GetPosition`，VBA 中转；12 分量，同 Position 口径）。
    pub fn get_position(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetPosition")
    }

    /// 设置断面位置（`SetPosition(分量数组)`；12 分量）。
    pub fn set_position(&self, components: &[f64]) -> ComResult<()> {
        let args = Arg::SafeArray(components.iter().map(|v| Arg::F64(*v)).collect());
        self.com().call("SetPosition", &[args]).map(|_| ())
    }
}
