//! 草图基础设施：`Sketches` / `Sketch` / `Factory2D` / `Point2D` / `Line2D` /
//! `Constraints` / `Constraint` / `Dimension`。
//!
//! vba_bbox_generate 的矩形+约束+投影全走这条链：HybridBody.HybridSketches.Add
//! → Sketch.OpenEdition(Factory2D) → 画点线/投影/约束 → CloseEdition。
//! 约束类型常量与 CATIA 枚举一致（catCstTypeDistance=1、Length=5、
//! Horizontality=10、Verticality=13、Parallelism=8，取自 VBA 原宏）。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::part::Entity;
use super::reference::Reference;

com_wrapper!(Sketches);
com_wrapper!(Sketch);
com_wrapper!(Factory2D);
com_wrapper!(Point2D);
com_wrapper!(Line2D);
com_wrapper!(Constraints);
com_wrapper!(Constraint);
com_wrapper!(Dimension);

/// catCstTypeDistance。
pub const CAT_CST_TYPE_DISTANCE: i32 = 1;
/// catCstTypeLength（VBA 原宏用 5 —— 与部分文档的 3 不同，按原宏原样）。
pub const CAT_CST_TYPE_LENGTH: i32 = 5;
/// catCstTypeHorizontality。
pub const CAT_CST_TYPE_HORONTALITY: i32 = 10;
/// catCstTypeVerticality。
pub const CAT_CST_TYPE_VERTICALITY: i32 = 13;
/// catCstTypeParallelism。
pub const CAT_CST_TYPE_PARALLELISM: i32 = 8;

impl ComCollection for Sketches {
    type Item = Sketch;
}

impl Sketches {
    /// 在指定平面/坐标系引用上新建草图（Python 侧混传 Reference 或平面特征）。
    pub fn add(&self, plane: &impl ComObject) -> ComResult<Sketch> {
        Ok(Sketch::new(
            self.com()
                .call("Add", &[Arg::Obj(plane.com().clone())])?
                .as_obj()?,
        ))
    }
}

impl Sketch {
    /// 打开草图编辑，返回 2D 工厂。
    pub fn open_edition(&self) -> ComResult<Factory2D> {
        Ok(Factory2D::new(
            self.com().call("OpenEdition", &[])?.as_obj()?,
        ))
    }

    /// 关闭草图编辑。
    pub fn close_edition(&self) -> ComResult<()> {
        self.com().call("CloseEdition", &[]).map(|_| ())
    }

    /// 草图几何元素集合（`Item("AbsoluteAxis")` 取绝对轴）。
    pub fn geometric_elements(&self) -> ComResult<super::part::Entities> {
        Ok(super::part::Entities::new(
            self.com().get("GeometricElements")?.as_obj()?,
        ))
    }

    /// 草图约束集合。
    pub fn constraints(&self) -> ComResult<Constraints> {
        Ok(Constraints::new(self.com().get("Constraints")?.as_obj()?))
    }
}

impl Factory2D {
    /// 2D 点。
    pub fn create_point(&self, x: f64, y: f64) -> ComResult<Point2D> {
        Ok(Point2D::new(
            self.com()
                .call("CreatePoint", &[Arg::F64(x), Arg::F64(y)])?
                .as_obj()?,
        ))
    }

    /// 2D 线段。
    pub fn create_line(&self, x1: f64, y1: f64, x2: f64, y2: f64) -> ComResult<Line2D> {
        Ok(Line2D::new(
            self.com()
                .call(
                    "CreateLine",
                    &[Arg::F64(x1), Arg::F64(y1), Arg::F64(x2), Arg::F64(y2)],
                )?
                .as_obj()?,
        ))
    }

    /// 把 3D 参考投影进草图平面。
    pub fn create_projection(&self, element: &Reference) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call("CreateProjection", &[Arg::Obj(element.com().clone())])?
                .as_obj()?,
        ))
    }
}

impl Point2D {
    /// 控制报告编号（VBA 原宏用 1..10 标注轴/点/线的引用顺序）。
    pub fn set_report_name(&self, value: i32) -> ComResult<()> {
        self.com().put("ReportName", Arg::I32(value))
    }
}

impl Line2D {
    pub fn set_report_name(&self, value: i32) -> ComResult<()> {
        self.com().put("ReportName", Arg::I32(value))
    }

    pub fn set_start_point(&self, point: &impl ComObject) -> ComResult<()> {
        self.com().put("StartPoint", Arg::Obj(point.com().clone()))
    }

    pub fn set_end_point(&self, point: &impl ComObject) -> ComResult<()> {
        self.com().put("EndPoint", Arg::Obj(point.com().clone()))
    }
}

impl Constraints {
    /// 单元素约束（水平/垂直/长度等）。
    pub fn add_mono_elt_cst(&self, cst_type: i32, element: &Reference) -> ComResult<Constraint> {
        Ok(Constraint::new(
            self.com()
                .call(
                    "AddMonoEltCst",
                    &[Arg::I32(cst_type), Arg::Obj(element.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 双元素约束（距离/平行等）。
    pub fn add_bi_elt_cst(
        &self,
        cst_type: i32,
        first: &Reference,
        second: &Reference,
    ) -> ComResult<Constraint> {
        Ok(Constraint::new(
            self.com()
                .call(
                    "AddBiEltCst",
                    &[
                        Arg::I32(cst_type),
                        Arg::Obj(first.com().clone()),
                        Arg::Obj(second.com().clone()),
                    ],
                )?
                .as_obj()?,
        ))
    }
}

impl Constraint {
    /// 约束的尺寸对象（距离/长度约束的数值载体）。
    pub fn dimension(&self) -> ComResult<Dimension> {
        Ok(Dimension::new(self.com().get("Dimension")?.as_obj()?))
    }
}

impl Dimension {
    /// 尺寸值（mm；读出 dx/dy 测量结果 / 写 0 贴合极值点）。
    pub fn value(&self) -> ComResult<f64> {
        self.com().get("Value")?.as_f64()
    }

    pub fn set_value(&self, value: f64) -> ComResult<()> {
        self.com().put("Value", Arg::F64(value))
    }
}
