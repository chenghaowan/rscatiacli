//! 机械模型补充族（pycatia `mec_mod_interfaces` 阶段 2 补全）。
//!
//! 拓扑读取（`Boundary`/`Edge`/`Face`/`Vertex` 及其子类型，多为标记类型、
//! 经 `Part::create_reference_from_b_rep_name` 等入口取得）、有序几何集
//! （`OrderedGeometricalSet(s)`）、几何元素集合（`GeometricElements`）、
//! 特征实例（`ShapeInstance` / `HybridShapeInstance`）、`Solid`、
//! `FixTogether(s)` 与 GSD 基类 `HybridShape`。
//!
//! 已在 [`super::part`] 的：`Part` / `Body` / `HybridBody` / `Bodies` /
//! `HybridBodies` / `Shapes` / `HybridShapes` / `OriginElements` /
//! `AxisSystem(s)` / `Entity`。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::part::{Bodies, Entities};
use super::product::Product;
use super::reference::Reference;
use super::sketch::Sketches;
use super::system_service::SystemService;

com_wrapper!(Boundary);
com_wrapper!(Edge);
com_wrapper!(Face);
com_wrapper!(Vertex);
com_wrapper!(BiDimFeatEdge);
com_wrapper!(MonoDimFeatEdge);
com_wrapper!(TriDimFeatEdge);
com_wrapper!(NotWireBoundaryMonoDimFeatVertex);
com_wrapper!(TriDimFeatVertexOrBiDimFeatVertex);
com_wrapper!(ZeroDimFeatVertexOrWireBoundaryMonoDimFeatVertex);
com_wrapper!(CylindricalFace);
com_wrapper!(PlanarFace);
com_wrapper!(RectilinearBiDimFeatEdge);
com_wrapper!(RectilinearMonoDimFeatEdge);
com_wrapper!(RectilinearTriDimFeatEdge);
com_wrapper!(Shape);
com_wrapper!(Solid);
com_wrapper!(Factory);
com_wrapper!(GeometricElements);
com_wrapper!(OrderedGeometricalSet);
com_wrapper!(OrderedGeometricalSets);
com_wrapper!(FixTogethers);
com_wrapper!(FixTogether);
com_wrapper!(HybridShape);
com_wrapper!(ShapeInstance);
com_wrapper!(HybridShapeInstance);
com_wrapper!(PartServices);

impl ComCollection for GeometricElements {
    type Item = ComObj;
}

impl ComCollection for OrderedGeometricalSets {
    type Item = OrderedGeometricalSet;
}

impl ComCollection for FixTogethers {
    type Item = FixTogether;
}

impl OrderedGeometricalSets {
    /// 新建有序几何集（`Add`）。
    pub fn add(&self) -> ComResult<OrderedGeometricalSet> {
        Ok(OrderedGeometricalSet::new(
            self.com().call("Add", &[])?.as_obj()?,
        ))
    }
}

impl OrderedGeometricalSet {
    /// 内含实体体集合（`Bodies`）。
    pub fn bodies(&self) -> ComResult<Bodies> {
        Ok(Bodies::new(self.com().get("Bodies")?.as_obj()?))
    }

    /// 混合形状集合（`HybridShapes`）。
    pub fn hybrid_shapes(&self) -> ComResult<Entities> {
        Ok(Entities::new(self.com().get("HybridShapes")?.as_obj()?))
    }

    /// 子有序几何集集合（`OrderedGeometricalSets`）。
    pub fn ordered_geometrical_sets(&self) -> ComResult<OrderedGeometricalSets> {
        Ok(OrderedGeometricalSets::new(
            self.com().get("OrderedGeometricalSets")?.as_obj()?,
        ))
    }

    /// 有序草图集合（`OrderedSketches`）。
    pub fn ordered_sketches(&self) -> ComResult<Sketches> {
        Ok(Sketches::new(self.com().get("OrderedSketches")?.as_obj()?))
    }

    /// 追加混合形状（`InsertHybridShape`）。
    pub fn insert_hybrid_shape(&self, shape: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("InsertHybridShape", &[Arg::Obj(shape.com().clone())])
            .map(|_| ())
    }
}

impl FixTogethers {
    /// 新建固联（`Add`）。
    pub fn add(&self) -> ComResult<FixTogether> {
        Ok(FixTogether::new(self.com().call("Add", &[])?.as_obj()?))
    }

    /// 删除固联（`Remove`，按序号）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }
}

impl FixTogether {
    prop_get!(fix_togethers_count, "FixTogethersCount", i32);
    prop_get!(products_count, "ProductsCount", i32);

    /// 追加子固联（`AddFixTogether`）。
    pub fn add_fix_together(&self, fix_together: &FixTogether) -> ComResult<()> {
        self.com()
            .call("AddFixTogether", &[Arg::Obj(fix_together.com().clone())])
            .map(|_| ())
    }

    /// 追加参与产品（`AddProduct`）。
    pub fn add_product(&self, product: &Product) -> ComResult<()> {
        self.com()
            .call("AddProduct", &[Arg::Obj(product.com().clone())])
            .map(|_| ())
    }

    /// 按序号取子固联（`GetFixTogether`）。
    pub fn get_fix_together(&self, index: i32) -> ComResult<FixTogether> {
        Ok(FixTogether::new(
            self.com()
                .call("GetFixTogether", &[Arg::I32(index)])?
                .as_obj()?,
        ))
    }

    /// 按序号取参与产品（`GetProduct`）。
    pub fn get_product(&self, index: i32) -> ComResult<Product> {
        Ok(Product::new(
            self.com()
                .call("GetProduct", &[Arg::I32(index)])?
                .as_obj()?,
        ))
    }

    /// 撤销子固联（`RemoveFixTogether`）。
    pub fn remove_fix_together(&self, index: i32) -> ComResult<()> {
        self.com()
            .call("RemoveFixTogether", &[Arg::I32(index)])
            .map(|_| ())
    }

    /// 撤销参与产品（`RemoveProduct`）。
    pub fn remove_product(&self, index: i32) -> ComResult<()> {
        self.com()
            .call("RemoveProduct", &[Arg::I32(index)])
            .map(|_| ())
    }
}

impl HybridShape {
    /// 厚度子对象（`Thickness`；CAA 口径为 HybridShape）。
    pub fn thickness(&self) -> ComResult<HybridShape> {
        Ok(HybridShape::new(self.com().get("Thickness")?.as_obj()?))
    }

    /// 追加混合形状（`AppendHybridShape`）。
    pub fn append_hybrid_shape(&self, shape: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("AppendHybridShape", &[Arg::Obj(shape.com().clone())])
            .map(|_| ())
    }

    /// 计算（`Compute`）。
    pub fn compute(&self) -> ComResult<()> {
        self.com().call("Compute", &[]).map(|_| ())
    }
}

impl Solid {
    /// 位姿对象（`Move`；Move 类型属 in_interfaces，阶段 4 补齐，先回原始对象）。
    pub fn r#move(&self) -> ComResult<ComObj> {
        self.com().get("Move")?.as_obj()
    }

    /// 源元素（`SourceElement`；装配上下文里的参考几何）。
    pub fn source_element(&self) -> ComResult<Reference> {
        Ok(Reference::new(self.com().get("SourceElement")?.as_obj()?))
    }

    /// 源产品（`SourceProduct`）。
    pub fn source_product(&self) -> ComResult<ComObj> {
        self.com().get("SourceProduct")?.as_obj()
    }
}

impl CylindricalFace {
    /// 轴向分量（`GetDirection`，VBA 中转）。
    pub fn get_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetDirection")
    }

    /// 轴上一点分量（`GetOrigin`，VBA 中转）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetOrigin")
    }
}

impl PlanarFace {
    /// 第一轴分量（`GetFirstAxis`，VBA 中转）。
    pub fn get_first_axis(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetFirstAxis")
    }

    /// 第二轴分量（`GetSecondAxis`，VBA 中转）。
    pub fn get_second_axis(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetSecondAxis")
    }

    /// 原点分量（`GetOrigin`，VBA 中转）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetOrigin")
    }
}

impl RectilinearBiDimFeatEdge {
    /// 方向分量（`GetDirection`，VBA 中转）。
    pub fn get_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetDirection")
    }

    /// 起点分量（`GetOrigin`，VBA 中转）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetOrigin")
    }
}

impl RectilinearMonoDimFeatEdge {
    /// 方向分量（`GetDirection`，VBA 中转）。
    pub fn get_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetDirection")
    }

    /// 起点分量（`GetOrigin`，VBA 中转）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetOrigin")
    }
}

impl RectilinearTriDimFeatEdge {
    /// 方向分量（`GetDirection`，VBA 中转）。
    pub fn get_direction(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetDirection")
    }

    /// 起点分量（`GetOrigin`，VBA 中转）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        ss.direction_out_param(self.com(), "GetOrigin")
    }
}

/// `ShapeInstance` / `HybridShapeInstance` 共有的实例化成员面
/// （put/get 输入输出与参数；成员名与 pycatia 一致）。
macro_rules! instance_surface {
    ($t:ident) => {
        impl $t {
            prop_get!(inputs_count, "InputsCount", i32);
            prop_get!(outputs_count, "OutputsCount", i32);
            prop_get!(parameters_count, "ParametersCount", i32);

            /// 按名取输入（`GetInput`）。
            pub fn get_input(&self, name: &str) -> ComResult<ComObj> {
                self.com()
                    .call("GetInput", &[Arg::Str(name.to_string())])?
                    .as_obj()
            }

            /// 按名取输入数据（`GetInputData`）。
            pub fn get_input_data(&self, name: &str) -> ComResult<ComObj> {
                self.com()
                    .call("GetInputData", &[Arg::Str(name.to_string())])?
                    .as_obj()
            }

            /// 按位置取输入数据（`GetInputDataFromPosition`）。
            pub fn get_input_data_from_position(&self, position: i32) -> ComResult<ComObj> {
                self.com()
                    .call("GetInputDataFromPosition", &[Arg::I32(position)])?
                    .as_obj()
            }

            /// 按位置取输入（`GetInputFromPosition`）。
            pub fn get_input_from_position(&self, position: i32) -> ComResult<ComObj> {
                self.com()
                    .call("GetInputFromPosition", &[Arg::I32(position)])?
                    .as_obj()
            }

            /// 按名取输出（`GetOutput`）。
            pub fn get_output(&self, name: &str) -> ComResult<ComObj> {
                self.com()
                    .call("GetOutput", &[Arg::Str(name.to_string())])?
                    .as_obj()
            }

            /// 按位置取输出（`GetOutputFromPosition`）。
            pub fn get_output_from_position(&self, position: i32) -> ComResult<ComObj> {
                self.com()
                    .call("GetOutputFromPosition", &[Arg::I32(position)])?
                    .as_obj()
            }

            /// 按名取参数（`GetParameter`）。
            pub fn get_parameter(&self, name: &str) -> ComResult<ComObj> {
                self.com()
                    .call("GetParameter", &[Arg::Str(name.to_string())])?
                    .as_obj()
            }

            /// 按位置取参数（`GetParameterFromPosition`）。
            pub fn get_parameter_from_position(&self, position: i32) -> ComResult<ComObj> {
                self.com()
                    .call("GetParameterFromPosition", &[Arg::I32(position)])?
                    .as_obj()
            }

            /// 按名写输入（`PutInput`）。
            pub fn put_input(&self, name: &str, input: &impl ComObject) -> ComResult<()> {
                self.com()
                    .call(
                        "PutInput",
                        &[Arg::Str(name.to_string()), Arg::Obj(input.com().clone())],
                    )
                    .map(|_| ())
            }

            /// 按名写输入数据（`PutInputData`）。
            pub fn put_input_data(&self, name: &str, input: &impl ComObject) -> ComResult<()> {
                self.com()
                    .call(
                        "PutInputData",
                        &[Arg::Str(name.to_string()), Arg::Obj(input.com().clone())],
                    )
                    .map(|_| ())
            }
        }
    };
}

instance_surface!(ShapeInstance);
instance_surface!(HybridShapeInstance);

impl PartServices {
    /// 取对象的子元素（`GetSubElements`；dimension: 0=顶点 1=边 2=面，
    /// duplicates 控制是否重复列出）。返回值口径未真机验证——若 COM 返回
    /// References 集合对象而非数组，走 `com()` 逃生舱。
    pub fn get_sub_elements(
        &self,
        object: &impl ComObject,
        sub_element_dimension: i32,
        duplicates: bool,
    ) -> ComResult<Vec<Reference>> {
        let raw = self
            .com()
            .call(
                "GetSubElements",
                &[
                    Arg::Obj(object.com().clone()),
                    Arg::I32(sub_element_dimension),
                    Arg::Bool(duplicates),
                ],
            )?
            .as_obj_array()?;
        Ok(raw.into_iter().map(Reference::new).collect())
    }
}
