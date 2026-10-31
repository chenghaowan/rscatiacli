//! FTA 3D 标注（`Part.AnnotationSets` 体系）。
//!
//! 对应 measure_harness 的长度注解：注解集复用已有的（没有才 `Add(标准)`）→
//! `UserSurfaces.Generate(曲面引用)` 造锚点（按序号命名跨运行复用，防止无限
//! 累积；UserSurface 没有删除方法，整删注解集才会连带清掉）→
//! `AnnotationFactory.CreateText(us)` → `Text().Text = "xx.x mm"` → SetXY/Name。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObject};
use crate::error::ComResult;

use super::reference::Reference;

com_wrapper!(AnnotationSets);
com_wrapper!(AnnotationSet);
com_wrapper!(Annotations);
com_wrapper!(Annotation);
com_wrapper!(AnnotationFactory);
com_wrapper!(AnnotationText);
com_wrapper!(UserSurfaces);
com_wrapper!(UserSurface);

impl ComCollection for AnnotationSets {
    type Item = AnnotationSet;
}

impl ComCollection for Annotations {
    type Item = Annotation;
}

impl ComCollection for UserSurfaces {
    type Item = UserSurface;
}

impl AnnotationSets {
    /// 新建注解集（standard 如 "ISO"）。
    pub fn add(&self, standard: &str) -> ComResult<AnnotationSet> {
        Ok(AnnotationSet::new(
            self.com()
                .call("Add", &[Arg::Str(standard.to_string())])?
                .as_obj()?,
        ))
    }
}

impl AnnotationSet {
    /// 注解显示开关（截图前关掉: SwitchOn=False 时不渲染视图虚线框/旧注解）。
    pub fn switch_on(&self) -> ComResult<bool> {
        self.com().get("SwitchOn")?.as_bool()
    }

    pub fn set_switch_on(&self, value: bool) -> ComResult<()> {
        self.com().put("SwitchOn", Arg::Bool(value))
    }

    pub fn annotations(&self) -> ComResult<Annotations> {
        Ok(Annotations::new(self.com().get("Annotations")?.as_obj()?))
    }

    pub fn annotation_factory(&self) -> ComResult<AnnotationFactory> {
        Ok(AnnotationFactory::new(
            self.com().get("AnnotationFactory")?.as_obj()?,
        ))
    }

    /// 宿主文档（整删注解集时经 Parent 拿 Selection）。
    pub fn parent(&self) -> ComResult<crate::dispatch::ComObj> {
        self.com().get("Parent")?.as_obj()
    }
}

impl Annotation {
    /// 文本对象（`Text()` 方法返回 AnnotationText, 其 Text 属性才是内容）。
    pub fn text_object(&self) -> ComResult<AnnotationText> {
        Ok(AnnotationText::new(self.com().call("Text", &[])?.as_obj()?))
    }

    /// 3D 视图里的标注位置（局部坐标; 建完通常 SetXY(0,0) 放锚点处）。
    pub fn set_xy(&self, x: f64, y: f64) -> ComResult<()> {
        self.com()
            .call("SetXY", &[Arg::F64(x), Arg::F64(y)])
            .map(|_| ())
    }
}

impl AnnotationText {
    pub fn set_text(&self, value: &str) -> ComResult<()> {
        self.com().put("Text", Arg::Str(value.to_string()))
    }

    pub fn text(&self) -> ComResult<String> {
        self.com().get("Text")?.as_string()
    }
}

impl AnnotationFactory {
    /// 在 UserSurface 锚点上建文字注解。
    pub fn create_text(&self, user_surface: &UserSurface) -> ComResult<Annotation> {
        Ok(Annotation::new(
            self.com()
                .call("CreateText", &[Arg::Obj(user_surface.com().clone())])?
                .as_obj()?,
        ))
    }
}

impl UserSurfaces {
    /// 在曲面上生成锚点（没有删除方法且不随注解删除回收 —— 调用方须按名字复用）。
    pub fn generate(&self, reference: &Reference) -> ComResult<UserSurface> {
        Ok(UserSurface::new(
            self.com()
                .call("Generate", &[Arg::Obj(reference.com().clone())])?
                .as_obj()?,
        ))
    }
}
