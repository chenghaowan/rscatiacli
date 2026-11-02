//! 知识工程族（pycatia `knowledge_interfaces`，阶段 5）。
//!
//! 关系（`Relations`/`Relation`/`Formula`/`Rule`/`Check`/`DesignTable`/
//! `SetOfEquation`/`ConstraintSatisfaction`/`Law`/`Loop`）、类型化参数
//! （`RealParam`/`IntParam`/`StrParam`/`BoolParam`/`EnumParam`/`Length`/
//! `Angle`/`Dimension`/`ListParameter`）、集合（`List`/`FreeParameters`/
//! `ParameterSet(s)`/`Units`）、优化（`Optimization(s)`/`OptimizationConstraint(s)`）、
//! `FeatureGenerator`。`Parameter`/`Parameters` 在 [`super::parameter`]。
//!
//! 注意：`SetOfEquation` 的 `GetSymbolcTransformations` / `UseSymbolcTransformations`
//! 拼写是 CAA/COM 原样（历史笔误），不改名。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject};
use crate::error::ComResult;

use super::parameter::{Parameter, Parameters};
use super::system_service::SystemService;

com_wrapper!(KnowledgeObject);
com_wrapper!(KnowledgeActivateObject);
com_wrapper!(Relation);
com_wrapper!(Formula);
com_wrapper!(Rule);
com_wrapper!(Check);
com_wrapper!(DesignTable);
com_wrapper!(SetOfEquation);
com_wrapper!(ConstraintSatisfaction);
com_wrapper!(Loop);
com_wrapper!(Law);
com_wrapper!(RealParam);
com_wrapper!(IntParam);
com_wrapper!(StrParam);
com_wrapper!(BoolParam);
com_wrapper!(EnumParam);
com_wrapper!(Length);
com_wrapper!(Angle);
com_wrapper!(Dimension);
com_wrapper!(ListParameter);
com_wrapper!(List);
com_wrapper!(FreeParameter);
com_wrapper!(FreeParameters);
com_wrapper!(ParameterSet);
com_wrapper!(ParameterSets);
com_wrapper!(Unit);
com_wrapper!(Units);
com_wrapper!(Relations);
com_wrapper!(FeatureGenerator);
com_wrapper!(Optimization);
com_wrapper!(Optimizations);
com_wrapper!(OptimizationConstraint);
com_wrapper!(OptimizationConstraints);

impl ComCollection for Relations {
    type Item = Relation;
}

impl ComCollection for FreeParameters {
    type Item = FreeParameter;
}

impl ComCollection for ParameterSets {
    type Item = ParameterSet;
}

impl ComCollection for Units {
    type Item = Unit;
}

impl ComCollection for List {
    type Item = ComObj;
}

impl ComCollection for Optimizations {
    type Item = Optimization;
}

impl ComCollection for OptimizationConstraints {
    type Item = OptimizationConstraint;
}

// ── 基类 ────────────────────────────────────────────────────────────

impl KnowledgeObject {
    prop_pair!(hidden, set_hidden, "Hidden", bool);
    prop_pair!(is_const, set_is_const, "IsConst", bool);
}

impl KnowledgeActivateObject {
    /// 是否激活（`Activated`）。
    pub fn activated(&self) -> ComResult<bool> {
        self.com().get("Activated")?.as_bool()
    }

    /// 激活（`Activate`）。
    pub fn activate(&self) -> ComResult<()> {
        self.com().call("Activate", &[]).map(|_| ())
    }

    /// 停用（`Deactivate`）。
    pub fn deactivate(&self) -> ComResult<()> {
        self.com().call("Deactivate", &[]).map(|_| ())
    }
}

impl Relation {
    prop_pair!(comment, set_comment, "Comment", str);

    /// 所属上下文对象（`Context`，原始对象）。
    pub fn context(&self) -> ComResult<ComObj> {
        self.com().get("Context")?.as_obj()
    }

    prop_get!(nb_in_parameters, "NBInParameters", i32);
    prop_get!(nb_out_parameters, "NBOutParameters", i32);

    /// 关系计算结果（`Value`，原始对象）。
    pub fn value(&self) -> ComResult<ComObj> {
        self.com().get("Value")?.as_obj()
    }

    /// 按序号取输入参数（`GetInParameter`）。
    pub fn get_in_parameter(&self, index: i32) -> ComResult<Parameter> {
        Ok(Parameter::new(
            self.com()
                .call("GetInParameter", &[Arg::I32(index)])?
                .as_obj()?,
        ))
    }

    /// 按序号取输出参数（`GetOutParameter`）。
    pub fn get_out_parameter(&self, index: i32) -> ComResult<Parameter> {
        Ok(Parameter::new(
            self.com()
                .call("GetOutParameter", &[Arg::I32(index)])?
                .as_obj()?,
        ))
    }

    /// 修改关系体（`Modify`，如公式表达式）。
    pub fn modify(&self, value: &str) -> ComResult<()> {
        self.com()
            .call("Modify", &[Arg::Str(value.to_string())])
            .map(|_| ())
    }

    /// 重命名（`Rename`）。
    pub fn rename(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("Rename", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }
}

// ── 关系子类 ────────────────────────────────────────────────────────

impl Relations {
    /// 新建公式（`CreateFormula`：输出参数 + 表达式体）。
    pub fn create_formula(
        &self,
        name: &str,
        comment: &str,
        output_parameter: &Parameter,
        formula_body: &str,
    ) -> ComResult<Formula> {
        Ok(Formula::new(
            self.com()
                .call(
                    "CreateFormula",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(comment.to_string()),
                        Arg::Obj(output_parameter.com().clone()),
                        Arg::Str(formula_body.to_string()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 新建检查（`CreateCheck`）。
    pub fn create_check(&self, name: &str, comment: &str, check_body: &str) -> ComResult<Check> {
        Ok(Check::new(
            self.com()
                .call(
                    "CreateCheck",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(comment.to_string()),
                        Arg::Str(check_body.to_string()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 新建法则（`CreateLaw`）。
    pub fn create_law(&self, name: &str, comment: &str, law_body: &str) -> ComResult<Law> {
        Ok(Law::new(
            self.com()
                .call(
                    "CreateLaw",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(comment.to_string()),
                        Arg::Str(law_body.to_string()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 新建规则（`CreateRule`；CAA Relations.CreateRule 存在但 pycatia fork
    /// 未列——名字按 CAA 口径）。
    pub fn create_rule(&self, name: &str, comment: &str, rule_body: &str) -> ComResult<Rule> {
        Ok(Rule::new(
            self.com()
                .call(
                    "CreateRule",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(comment.to_string()),
                        Arg::Str(rule_body.to_string()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 新建设计表（`CreateDesignTable`）。
    pub fn create_design_table(
        &self,
        name: &str,
        comment: &str,
        copy_mode: bool,
        sheet_path: &str,
    ) -> ComResult<DesignTable> {
        Ok(DesignTable::new(
            self.com()
                .call(
                    "CreateDesignTable",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(comment.to_string()),
                        Arg::Bool(copy_mode),
                        Arg::Str(sheet_path.to_string()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 新建横向设计表（`CreateHorizontalDesignTable`）。
    pub fn create_horizontal_design_table(
        &self,
        name: &str,
        comment: &str,
        copy_mode: bool,
        sheet_path: &str,
    ) -> ComResult<DesignTable> {
        Ok(DesignTable::new(
            self.com()
                .call(
                    "CreateHorizontalDesignTable",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(comment.to_string()),
                        Arg::Bool(copy_mode),
                        Arg::Str(sheet_path.to_string()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 新建方程组（`CreateSetOfEquations`）。
    pub fn create_set_of_equations(
        &self,
        name: &str,
        comment: &str,
        formula_body: &str,
    ) -> ComResult<SetOfEquation> {
        Ok(SetOfEquation::new(
            self.com()
                .call(
                    "CreateSetOfEquations",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(comment.to_string()),
                        Arg::Str(formula_body.to_string()),
                    ],
                )?
                .as_obj()?,
        ))
    }

    /// 按序号删除关系（`Remove`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }

    /// 优化集合入口（`Optimizations`）。
    pub fn optimizations(&self) -> ComResult<Optimizations> {
        Ok(Optimizations::new(
            self.com().get("Optimizations")?.as_obj()?,
        ))
    }

    /// 取某特征下的关系子集（`SubList`）。
    pub fn sub_list(&self, feature: &impl ComObject, recursively: bool) -> ComResult<Vec<ComObj>> {
        self.com()
            .call(
                "SubList",
                &[Arg::Obj(feature.com().clone()), Arg::Bool(recursively)],
            )?
            .as_obj_array()
    }
}

impl Check {
    /// 诊断结果（`Diagnosis`；CAA 口径为 BoolParam）。
    pub fn diagnosis(&self) -> ComResult<BoolParam> {
        Ok(BoolParam::new(self.com().get("Diagnosis")?.as_obj()?))
    }

    prop_pair!(severity, set_severity, "Severity", i32);
}

impl DesignTable {
    prop_get!(columns_nb, "ColumnsNb", i32);
    prop_pair!(configuration, set_configuration, "Configuration", i32);
    prop_get!(configurations_nb, "ConfigurationsNb", i32);
    prop_pair!(copy_mode, set_copy_mode, "CopyMode", bool);
    prop_pair!(file_path, set_file_path, "FilePath", str);

    /// 关联参数与列（`AddAssociation`）。
    pub fn add_association(&self, parameter: &Parameter, sheet_column: &str) -> ComResult<()> {
        self.com()
            .call(
                "AddAssociation",
                &[
                    Arg::Obj(parameter.com().clone()),
                    Arg::Str(sheet_column.to_string()),
                ],
            )
            .map(|_| ())
    }

    /// 追加一行（`AddNewRow`）。
    pub fn add_new_row(&self) -> ComResult<()> {
        self.com().call("AddNewRow", &[]).map(|_| ())
    }

    /// 读单元格文本（`CellAsString`）。
    pub fn cell_as_string(&self, row: i32, column: i32) -> ComResult<String> {
        self.com()
            .call("CellAsString", &[Arg::I32(row), Arg::I32(column)])?
            .as_string()
    }

    /// 解除列关联（`RemoveAssociation`）。
    pub fn remove_association(&self, sheet_column: &str) -> ComResult<()> {
        self.com()
            .call("RemoveAssociation", &[Arg::Str(sheet_column.to_string())])
            .map(|_| ())
    }

    /// 同步设计表（`Synchronize`）。
    pub fn synchronize(&self) -> ComResult<()> {
        self.com().call("Synchronize", &[]).map(|_| ())
    }
}

impl SetOfEquation {
    /// 最大计算时间（`GetMaxCalculationTime`）。
    pub fn get_max_calculation_time(&self) -> ComResult<i32> {
        self.com().call("GetMaxCalculationTime", &[])?.as_i32()
    }

    /// 精度（`GetPrecision`）。
    pub fn get_precision(&self) -> ComResult<f64> {
        self.com().call("GetPrecision", &[])?.as_f64()
    }

    /// 是否启用符号变换（`GetSymbolcTransformations`——CAA/COM 原拼写）。
    pub fn get_symbolc_transformations(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetSymbolcTransformations")
    }

    /// 是否弹停止对话框（`IsStopDialog`）。
    pub fn is_stop_dialog(&self) -> ComResult<bool> {
        self.com().call("IsStopDialog", &[])?.as_bool()
    }

    /// 设最大计算时间（`SetMaxCalculationTime`）。
    pub fn set_max_calculation_time(&self, max_time: i32) -> ComResult<()> {
        self.com()
            .call("SetMaxCalculationTime", &[Arg::I32(max_time)])
            .map(|_| ())
    }

    /// 设输入参数（`SetParameterAsInput`）。
    pub fn set_parameter_as_input(&self, parameter: &Parameter) -> ComResult<()> {
        self.com()
            .call("SetParameterAsInput", &[Arg::Obj(parameter.com().clone())])
            .map(|_| ())
    }

    /// 设输出参数（`SetParameterAsOutput`）。
    pub fn set_parameter_as_output(&self, parameter: &Parameter) -> ComResult<()> {
        self.com()
            .call("SetParameterAsOutput", &[Arg::Obj(parameter.com().clone())])
            .map(|_| ())
    }

    /// 设精度（`SetPrecision`）。
    pub fn set_precision(&self, eps: f64) -> ComResult<()> {
        self.com()
            .call("SetPrecision", &[Arg::F64(eps)])
            .map(|_| ())
    }

    /// 是否用停止对话框（`UseStopDialog`）。
    pub fn use_stop_dialog(&self, used: bool) -> ComResult<()> {
        self.com()
            .call("UseStopDialog", &[Arg::Bool(used)])
            .map(|_| ())
    }

    /// 是否用符号变换（`UseSymbolcTransformations`——CAA/COM 原拼写）。
    pub fn use_symbolc_transformations(&self, gauss: bool) -> ComResult<()> {
        self.com()
            .call("UseSymbolcTransformations", &[Arg::Bool(gauss)])
            .map(|_| ())
    }
}

impl ConstraintSatisfaction {
    /// 求解（`Solve`）。
    pub fn solve(&self) -> ComResult<()> {
        self.com().call("Solve", &[]).map(|_| ())
    }
}

impl Law {
    /// 追加形式参数（`AddFormalParameter`）。
    pub fn add_formal_parameter(&self, name: &str, magnitude: &str) -> ComResult<()> {
        self.com()
            .call(
                "AddFormalParameter",
                &[Arg::Str(name.to_string()), Arg::Str(magnitude.to_string())],
            )
            .map(|_| ())
    }

    /// 移除形式参数（`RemoveFormalParameter`）。
    pub fn remove_formal_parameter(&self, name: &str) -> ComResult<()> {
        self.com()
            .call("RemoveFormalParameter", &[Arg::Str(name.to_string())])
            .map(|_| ())
    }
}

// ── 类型化参数 ──────────────────────────────────────────────────────

impl RealParam {
    prop_pair!(value, set_value, "Value", f64);
    prop_pair!(
        maximum_tolerance,
        set_maximum_tolerance,
        "MaximumTolerance",
        f64
    );
    prop_pair!(
        minimum_tolerance,
        set_minimum_tolerance,
        "MinimumTolerance",
        f64
    );
    prop_pair!(range_max, set_range_max, "RangeMax", f64);
    prop_pair!(
        range_max_validity,
        set_range_max_validity,
        "RangeMaxValidity",
        i32
    );
    prop_pair!(range_min, set_range_min, "RangeMin", f64);
    prop_pair!(
        range_min_validity,
        set_range_min_validity,
        "RangeMinValidity",
        i32
    );

    /// 枚举值（`GetEnumerateValues`，VBA 中转）。
    pub fn get_enumerate_values(&self, ss: &SystemService) -> ComResult<Vec<f64>> {
        ss.numeric_out_param(self.com(), "GetEnumerateValues")
    }

    /// 枚举值个数（`GetEnumerateValuesSize`）。
    pub fn get_enumerate_values_size(&self) -> ComResult<i32> {
        self.com().call("GetEnumerateValuesSize", &[])?.as_i32()
    }

    /// 值是否相等（`IsEqualTo`）。
    pub fn is_equal_to(&self, value: f64) -> ComResult<bool> {
        self.com().call("IsEqualTo", &[Arg::F64(value)])?.as_bool()
    }

    /// 设枚举值（`SetEnumerateValues`）。
    pub fn set_enumerate_values(&self, values: &[f64]) -> ComResult<()> {
        let args = Arg::SafeArray(values.iter().map(|v| Arg::F64(*v)).collect());
        self.com().call("SetEnumerateValues", &[args]).map(|_| ())
    }

    /// 清空枚举值（`SuppressEnumerateValues`）。
    pub fn suppress_enumerate_values(&self) -> ComResult<()> {
        self.com().call("SuppressEnumerateValues", &[]).map(|_| ())
    }
}

impl IntParam {
    prop_pair!(value, set_value, "Value", i32);
    prop_pair!(range_max, set_range_max, "RangeMax", i32);
    prop_pair!(
        range_max_validity,
        set_range_max_validity,
        "RangeMaxValidity",
        i32
    );
    prop_pair!(range_min, set_range_min, "RangeMin", i32);
    prop_pair!(
        range_min_validity,
        set_range_min_validity,
        "RangeMinValidity",
        i32
    );

    /// 枚举值（`GetEnumerateValues`，VBA 中转）。
    pub fn get_enumerate_values(&self, ss: &SystemService) -> ComResult<Vec<i32>> {
        Ok(ss
            .numeric_out_param(self.com(), "GetEnumerateValues")?
            .into_iter()
            .map(|v| v as i32)
            .collect())
    }

    /// 枚举值个数（`GetEnumerateValuesSize`）。
    pub fn get_enumerate_values_size(&self) -> ComResult<i32> {
        self.com().call("GetEnumerateValuesSize", &[])?.as_i32()
    }

    /// 设枚举值（`SetEnumerateValues`）。
    pub fn set_enumerate_values(&self, values: &[i32]) -> ComResult<()> {
        let args = Arg::SafeArray(values.iter().map(|v| Arg::I32(*v)).collect());
        self.com().call("SetEnumerateValues", &[args]).map(|_| ())
    }

    /// 清空枚举值（`SuppressEnumerateValues`）。
    pub fn suppress_enumerate_values(&self) -> ComResult<()> {
        self.com().call("SuppressEnumerateValues", &[]).map(|_| ())
    }
}

impl StrParam {
    prop_pair!(value, set_value, "Value", str);

    /// 枚举值（`GetEnumerateValues`，VBA 中转字符串数组）。
    pub fn get_enumerate_values(&self, ss: &SystemService) -> ComResult<Vec<String>> {
        ss.strings_out_param(self.com(), "GetEnumerateValues")
    }

    /// 枚举值个数（`GetEnumerateValuesSize`）。
    pub fn get_enumerate_values_size(&self) -> ComResult<i32> {
        self.com().call("GetEnumerateValuesSize", &[])?.as_i32()
    }

    /// 设枚举值（`SetEnumerateValues`）。
    pub fn set_enumerate_values(&self, values: &[String]) -> ComResult<()> {
        let args = Arg::SafeArray(values.iter().map(|v| Arg::Str(v.clone())).collect());
        self.com().call("SetEnumerateValues", &[args]).map(|_| ())
    }

    /// 清空枚举值（`SuppressEnumerateValues`）。
    pub fn suppress_enumerate_values(&self) -> ComResult<()> {
        self.com().call("SuppressEnumerateValues", &[]).map(|_| ())
    }
}

impl EnumParam {
    prop_pair!(value_enum, set_value_enum, "Value", str);
}

impl BoolParam {
    prop_pair!(value, set_value, "Value", bool);
}

impl Dimension {
    /// 单位（`Unit`）。
    pub fn unit(&self) -> ComResult<Unit> {
        Ok(Unit::new(self.com().get("Unit")?.as_obj()?))
    }

    /// 值的格式化字符串（`ValueAsString`，带小数位/尾零控制）。
    pub fn value_as_string2(
        &self,
        nb_decimals: i32,
        show_trailing_zeros: bool,
    ) -> ComResult<String> {
        self.com()
            .call(
                "ValueAsString",
                &[Arg::I32(nb_decimals), Arg::Bool(show_trailing_zeros)],
            )?
            .as_string()
    }
}

impl ListParameter {
    /// 值列表（`ValueList`）。
    pub fn value_list(&self) -> ComResult<List> {
        Ok(List::new(self.com().get("ValueList")?.as_obj()?))
    }
}

impl List {
    /// 追加元素（`Add`）。
    pub fn add(&self, item: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("Add", &[Arg::Obj(item.com().clone())])
            .map(|_| ())
    }

    /// 按序号删除（`Remove`）。
    pub fn remove(&self, index: i32) -> ComResult<()> {
        self.com().call("Remove", &[Arg::I32(index)]).map(|_| ())
    }

    /// 重排（`Reorder`）。
    pub fn reorder(&self, index_current: i32, index_target: i32) -> ComResult<()> {
        self.com()
            .call(
                "Reorder",
                &[Arg::I32(index_current), Arg::I32(index_target)],
            )
            .map(|_| ())
    }

    /// 替换元素（`Replace`）。
    pub fn replace(&self, index: i32, item: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("Replace", &[Arg::I32(index), Arg::Obj(item.com().clone())])
            .map(|_| ())
    }
}

impl FreeParameter {
    prop_pair!(inf_range, set_inf_range, "InfRange", f64);
    prop_pair!(sup_range, set_sup_range, "SupRange", f64);
    prop_pair!(step, set_step, "Step", f64);
    prop_pair!(parameter, set_parameter, "Parameter", RealParam);
}

impl FreeParameters {
    /// 追加自由参数（`AddFreeParameter`）。
    pub fn add_free_parameter(&self, parameter: &RealParam) -> ComResult<()> {
        self.com()
            .call("AddFreeParameter", &[Arg::Obj(parameter.com().clone())])
            .map(|_| ())
    }

    /// 按序号移除（`RemoveFreeParameter`）。
    pub fn remove_free_parameter(&self, index: i32) -> ComResult<()> {
        self.com()
            .call("RemoveFreeParameter", &[Arg::I32(index)])
            .map(|_| ())
    }
}

// ── 参数集 / 单位 ───────────────────────────────────────────────────

impl ParameterSet {
    /// 全部参数（含子集，`AllParameters`）。
    pub fn all_parameters(&self) -> ComResult<Parameters> {
        Ok(Parameters::new(self.com().get("AllParameters")?.as_obj()?))
    }

    /// 直接参数（`DirectParameters`）。
    pub fn direct_parameters(&self) -> ComResult<Parameters> {
        Ok(Parameters::new(
            self.com().get("DirectParameters")?.as_obj()?,
        ))
    }

    /// 子参数集（`ParameterSets`）。
    pub fn parameter_sets(&self) -> ComResult<ParameterSets> {
        Ok(ParameterSets::new(
            self.com().get("ParameterSets")?.as_obj()?,
        ))
    }
}

impl ParameterSets {
    /// 新建参数集（`CreateSet`）。
    pub fn create_set(&self, name: &str) -> ComResult<ParameterSet> {
        Ok(ParameterSet::new(
            self.com()
                .call("CreateSet", &[Arg::Str(name.to_string())])?
                .as_obj()?,
        ))
    }
}

impl Unit {
    prop_get!(magnitude, "Magnitude", str);
    prop_get!(symbol, "Symbol", str);

    /// MKS → 本单位（`ConvertFromMKS`）。
    pub fn convert_from_mks(&self, value_in_mks: f64) -> ComResult<f64> {
        self.com()
            .call("ConvertFromMKS", &[Arg::F64(value_in_mks)])?
            .as_f64()
    }

    /// 存储单位 → 本单位（`ConvertFromStorageUnit`）。
    pub fn convert_from_storage_unit(&self, value: f64) -> ComResult<f64> {
        self.com()
            .call("ConvertFromStorageUnit", &[Arg::F64(value)])?
            .as_f64()
    }

    /// 本单位 → MKS（`ConvertToMKS`）。
    pub fn convert_to_mks(&self, value: f64) -> ComResult<f64> {
        self.com()
            .call("ConvertToMKS", &[Arg::F64(value)])?
            .as_f64()
    }

    /// 本单位 → 存储单位（`ConvertToStorageUnit`）。
    pub fn convert_to_storage_unit(&self, value: f64) -> ComResult<f64> {
        self.com()
            .call("ConvertToStorageUnit", &[Arg::F64(value)])?
            .as_f64()
    }
}

// ── 优化 / 特征生成器 ───────────────────────────────────────────────

impl Optimizations {
    /// 新建优化（`CreateOptimization`）。
    pub fn create_optimization(&self) -> ComResult<Optimization> {
        Ok(Optimization::new(
            self.com().call("CreateOptimization", &[])?.as_obj()?,
        ))
    }

    /// 新建约束满足（`CreateConstraintsSatisfaction`）。
    pub fn create_constraints_satisfaction(
        &self,
        name: &str,
        comment: &str,
        formula_body: &str,
    ) -> ComResult<ConstraintSatisfaction> {
        Ok(ConstraintSatisfaction::new(
            self.com()
                .call(
                    "CreateConstraintsSatisfaction",
                    &[
                        Arg::Str(name.to_string()),
                        Arg::Str(comment.to_string()),
                        Arg::Str(formula_body.to_string()),
                    ],
                )?
                .as_obj()?,
        ))
    }
}

impl Optimization {
    prop_pair!(algorithm_type, set_algorithm_type, "AlgorithmType", i32);
    prop_pair!(conv_speed, set_conv_speed, "ConvSpeed", i32);
    prop_pair!(max_evals_nb, set_max_evals_nb, "MaxEvalsNb", i32);
    prop_pair!(
        max_evals_wo_improvement,
        set_max_evals_wo_improvement,
        "MaxEvalsWoImprovement",
        i32
    );
    prop_pair!(max_time, set_max_time, "MaxTime", i32);
    prop_pair!(
        optimization_type,
        set_optimization_type,
        "OptimizationType",
        i32
    );
    prop_pair!(
        use_max_evals_wo_improvement,
        set_use_max_evals_wo_improvement,
        "UseMaxEvalsWoImprovement",
        bool
    );
    prop_pair!(use_max_time, set_use_max_time, "UseMaxTime", bool);
    prop_pair!(
        objective_parameter,
        set_objective_parameter,
        "ObjectiveParameter",
        RealParam
    );

    /// 约束集合（`Constraints`）。
    pub fn constraints(&self) -> ComResult<OptimizationConstraints> {
        Ok(OptimizationConstraints::new(
            self.com().get("Constraints")?.as_obj()?,
        ))
    }

    /// 自由参数集合（`FreeParameters`）。
    pub fn free_parameters(&self) -> ComResult<FreeParameters> {
        Ok(FreeParameters::new(
            self.com().get("FreeParameters")?.as_obj()?,
        ))
    }

    /// 目标值（`TargetValue`，原始对象）。
    pub fn target_value(&self) -> ComResult<ComObj> {
        self.com().get("TargetValue")?.as_obj()
    }

    /// 运行优化（`Run`）。
    pub fn run(&self, with_stop_dialog: bool) -> ComResult<()> {
        self.com()
            .call("Run", &[Arg::Bool(with_stop_dialog)])
            .map(|_| ())
    }
}

impl OptimizationConstraints {
    /// 按表达式追加约束（`AddConstraint`）。
    pub fn add_constraint(&self, constraint_expression: &str) -> ComResult<OptimizationConstraint> {
        Ok(OptimizationConstraint::new(
            self.com()
                .call(
                    "AddConstraint",
                    &[Arg::Str(constraint_expression.to_string())],
                )?
                .as_obj()?,
        ))
    }

    /// 按序号删除约束（`RemoveConstraint`）。
    pub fn remove_constraint(&self, index: i32) -> ComResult<()> {
        self.com()
            .call("RemoveConstraint", &[Arg::I32(index)])
            .map(|_| ())
    }
}

impl OptimizationConstraint {
    /// 距满足值的距离（`DistanceToSatisfaction`）。
    pub fn distance_to_satisfaction(&self) -> ComResult<f64> {
        self.com().get("DistanceToSatisfaction")?.as_f64()
    }

    prop_pair!(precision, set_precision, "Precision", f64);

    /// 满足度（`Satisfaction`）。
    pub fn satisfaction(&self) -> ComResult<f64> {
        self.com().get("Satisfaction")?.as_f64()
    }
}

impl FeatureGenerator {
    prop_get!(nb_generated_features, "NbGeneratedFeatures", i32);
    prop_pair!(script, set_script, "Script", str);

    /// 生成特征（`Generate`）。
    pub fn generate(&self, context: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("Generate", &[Arg::Obj(context.com().clone())])
            .map(|_| ())
    }

    /// 按输入数组生成（`GenerateInContext`）。
    pub fn generate_in_context(&self, inputs: &[&dyn ComObject]) -> ComResult<()> {
        let args = Arg::SafeArray(inputs.iter().map(|o| Arg::Obj(o.com().clone())).collect());
        self.com().call("GenerateInContext", &[args]).map(|_| ())
    }

    /// 从文件加载脚本（`LoadScriptFromFilePath`）。
    pub fn load_script_from_file_path(&self, file_path: &str) -> ComResult<()> {
        self.com()
            .call("LoadScriptFromFilePath", &[Arg::Str(file_path.to_string())])
            .map(|_| ())
    }
}
