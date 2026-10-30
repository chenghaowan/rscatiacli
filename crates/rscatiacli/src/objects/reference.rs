//! `Reference`：指向几何对象（Body / HybridShape / datum ...）的引用句柄。
//!
//! 由 [`Part::create_reference_from_object`](super::part::Part) 创建，供
//! `HybridShapeFactory` datum 化与 `SPAWorkbench` 测量使用。成员按需补充。

com_wrapper!(Reference);
