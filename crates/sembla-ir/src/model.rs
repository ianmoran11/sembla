use serde::{Deserialize, Serialize};

/// A complete Sembla intermediate-representation model.
///
/// Tables and parameters define the columnar state contract described in
/// `DESIGN.md` §4.1. `dt` is the model-time tick width from §4.3. Parameter
/// values are declarations here and are never inlined into [`Expr`] nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub name: String,
    pub dt: f64,
    pub params: Vec<ParamDecl>,
    pub boxes: Vec<Box>,
    pub wires: Vec<Wire>,
}

/// A per-run constant parameter declaration (`DESIGN.md` §4.1).
///
/// Expressions refer to parameters by name. Parameter values are never
/// inlined into the IR: the run contract remains seed + IR + θ + level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamDecl {
    pub name: String,
    pub ty: ParamType,
    pub default: ParamValue,
    pub prior: Option<Prior>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    Real,
    Int,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParamValue {
    Real { value: f64 },
    Int { value: i64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prior {
    pub family: PriorFamily,
    pub args: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorFamily {
    Normal,
    LogNormal,
    Uniform,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Box {
    pub name: String,
    pub tables: Vec<Table>,
    pub transitions: Vec<Transition>,
    pub inputs: Vec<PortDecl>,
    pub outputs: Vec<OutputDecl>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub name: String,
    pub size_hint: u64,
    pub attrs: Vec<Attr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attr {
    pub name: String,
    pub ty: AttrType,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttrType {
    Real,
    Int,
    Enum { variants: Vec<String> },
    Ref { table: String },
}

/// A hazard-rate transition over one table (`DESIGN.md` §4.3).
///
/// Its guard is Boolean, its hazard is a real rate per model-time unit, and
/// effects update only the transition's current row in v0.1. Named parameters
/// remain [`Expr::Param`] references and are never inlined into the IR.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub name: String,
    pub table: String,
    pub guard: Expr,
    pub hazard: Expr,
    pub effects: Vec<Effect>,
    pub contests: Vec<ResourceClaim>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    SetAttr { attr: String, value: Expr },
}

/// A first-order, typed, allocation-free expression (`DESIGN.md` §4.2).
///
/// The language contains no recursion, user functions, or unbounded joins.
/// Parameters are referenced symbolically and their values are never inlined
/// into the IR.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    Real {
        value: f64,
    },
    Int {
        value: i64,
    },
    Bool {
        value: bool,
    },
    Enum {
        variant: String,
    },
    Param {
        name: String,
    },
    SelfAttr {
        name: String,
    },
    Add {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Sub {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Mul {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Div {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Eq {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Ne {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Lt {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Le {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Gt {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Ge {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    And {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Or {
        lhs: std::boxed::Box<Expr>,
        rhs: std::boxed::Box<Expr>,
    },
    Not {
        expr: std::boxed::Box<Expr>,
    },
    EnumIs {
        attr: String,
        variant: String,
    },
    Input {
        port: String,
        agg: Aggregate,
    },
    Agg {
        op: AggOp,
        table: String,
        on: AggJoin,
        filter: std::boxed::Box<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aggregate {
    pub op: AggOp,
    pub filter: Option<std::boxed::Box<Expr>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AggOp {
    Count,
    Sum { value: std::boxed::Box<Expr> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AggJoin {
    pub fk_attr: String,
    pub self_fk_attr: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortDecl {
    pub name: String,
    pub schema: Vec<Attr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputDecl {
    pub name: String,
    pub schema: Vec<Attr>,
    pub builder: OutputBuilder,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OutputBuilder {
    PerTable {
        table: String,
        fields: Vec<OutputField>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputField {
    pub name: String,
    pub op: AggOp,
    pub filter: Option<std::boxed::Box<Expr>>,
}

/// A declaration that a transition contests a Ref-typed entity.
///
/// As specified by `DESIGN.md` §5.1, at most one contestant for a resource
/// fires per tick. Race-time ordering uses sampled firing time; key ordering
/// uses the given expression in ascending order. To cover writes as required by
/// §5.1, a [`Effect::SetAttr`] targeting a Ref attribute must have a claim whose
/// resource is structurally equal to the effect's value expression; this claims
/// the entity being assigned. Parameter references inside either expression
/// remain symbolic and are never inlined into the IR.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceClaim {
    pub resource: Expr,
    pub ordering: ClaimOrdering,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimOrdering {
    RaceTime,
    Key { expr: Expr },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub from: WireEndpoint,
    pub to: WireEndpoint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireEndpoint {
    pub r#box: String,
    pub port: String,
}
