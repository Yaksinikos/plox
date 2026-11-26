////////////////////////////////////////////////////////////////////////
// EXPRESSIONS
////////////////////////////////////////////////////////////////////////

use std::fmt::Display;

use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::{PluginData, wild_contains, wild_contains_data};

// An expression may be evaluated against a load order
pub trait TExpression {
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Expression {
    Atomic(Atomic),
    ALL(ALL),
    ANY(ANY),
    NOT(NOT),
    DESC(DESC),
    SIZE(SIZE),
    VER(VER),
    GVER(GVER),
}

// pass-through
impl Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Atomic(x) => x.fmt(f),
            Self::ALL(x) => x.fmt(f),
            Self::ANY(x) => x.fmt(f),
            Self::NOT(x) => x.fmt(f),
            Self::DESC(x) => x.fmt(f),
            Self::SIZE(x) => x.fmt(f),
            Self::VER(x) => x.fmt(f),
            Self::GVER(x) => x.fmt(f),
        }
    }
}
impl TExpression for Expression {
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        match self {
            Self::Atomic(x) => x.eval(items),
            Self::ALL(x) => x.eval(items),
            Self::ANY(x) => x.eval(items),
            Self::NOT(x) => x.eval(items),
            Self::DESC(x) => x.eval(items),
            Self::SIZE(x) => x.eval(items),
            Self::VER(x) => x.eval(items),
            Self::GVER(x) => x.eval(items),
        }
    }
}

// conversions
impl From<Atomic> for Expression {
    fn from(val: Atomic) -> Self {
        Self::Atomic(val)
    }
}
impl From<ALL> for Expression {
    fn from(val: ALL) -> Self {
        Self::ALL(val)
    }
}
impl From<ANY> for Expression {
    fn from(val: ANY) -> Self {
        Self::ANY(val)
    }
}
impl From<NOT> for Expression {
    fn from(val: NOT) -> Self {
        Self::NOT(val)
    }
}
impl From<DESC> for Expression {
    fn from(val: DESC) -> Self {
        Self::DESC(val)
    }
}
impl From<SIZE> for Expression {
    fn from(val: SIZE) -> Self {
        Self::SIZE(val)
    }
}
impl From<VER> for Expression {
    fn from(val: VER) -> Self {
        Self::VER(val)
    }
}
impl From<GVER> for Expression {
    fn from(val: GVER) -> Self {
        Self::GVER(val)
    }
}

////////////////////////////////////////////////////////////////////////
// IMPLEMENTATIONS
////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////
// ATOMIC

/// The atomic expression (EXISTS)
/// atomics evaluate as true if the input list contains the item
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Atomic {
    pub item: String,
}

impl Atomic {
    #[must_use]
    pub fn get_item(&self) -> String {
        self.item.clone()
    }
}
impl TExpression for Atomic {
    /// atomics evaluate as true if the input list contains the item
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        wild_contains(
            &items.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
            &self.item,
        )
    }
}

impl From<&str> for Atomic {
    fn from(value: &str) -> Self {
        Self { item: value.into() }
    }
}
impl From<String> for Atomic {
    fn from(value: String) -> Self {
        Self { item: value }
    }
}

impl Display for Atomic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.item)
    }
}

////////////////////////////////////////////////////////////////////////
// ALL

/// The ALL expression
/// ALL evaluates as true if all expressions evaluate as true
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ALL {
    pub expressions: Vec<Expression>,
}
impl ALL {
    #[must_use]
    pub const fn new(expressions: Vec<Expression>) -> Self {
        Self { expressions }
    }
}
impl TExpression for ALL {
    /// ALL evaluates as true if all expressions evaluate as true
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        let mut result = true;
        let mut results: Vec<String> = vec![];

        for e in &self.expressions {
            if let Some(plugins) = e.eval(items) {
                results.extend(plugins);
            } else {
                // any failure can set it to false
                result = false;
            }
        }

        if result { Some(results) } else { None }
    }
}

impl Display for ALL {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[ALL {}]",
            self.expressions
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n\t")
        )
    }
}

////////////////////////////////////////////////////////////////////////
// ANY

/// The ANY expression (OR)
/// ANY evaluates as true if any expressions evaluates as true
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ANY {
    pub expressions: Vec<Expression>,
}
impl ANY {
    #[must_use]
    pub const fn new(expressions: Vec<Expression>) -> Self {
        Self { expressions }
    }
}
impl TExpression for ANY {
    // ANY evaluate as true if any expressions evaluates as true
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        let mut result = false;
        let mut results: Vec<String> = vec![];

        for e in &self.expressions {
            if let Some(plugins) = e.eval(items) {
                result = true;
                results.extend(plugins);
            }
        }

        if result { Some(results) } else { None }
    }
}

impl Display for ANY {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[ANY {}]",
            self.expressions
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n\t")
        )
    }
}

////////////////////////////////////////////////////////////////////////
// NOT

/// The NOT expression
/// NOT evaluates as true if the wrapped expression evaluates as true
#[derive(Debug, Serialize, Deserialize)]
pub struct NOT {
    pub expression: Box<Expression>,
}
impl NOT {
    #[must_use]
    pub fn new(expression: Expression) -> Self {
        Self {
            expression: Box::new(expression),
        }
    }
}
impl TExpression for NOT {
    // NOT evaluates as true if the wrapped expression evaluates as true
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        // NOT and resolving names
        match self.expression.eval(items) {
            Some(_) => None,
            None => Some(vec![self.to_string()]),
        }
    }
}
impl Clone for NOT {
    fn clone(&self) -> Self {
        Self {
            expression: self.expression.clone(),
        }
    }
}

impl Display for NOT {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[NOT {}]", self.expression.clone())
    }
}

////////////////////////////////////////////////////////////////////////
// DESC

/// The Desc predicate is a special predicate that matches strings in the header of a plugin with regular expressions.
/// [DESC /regex/ A.esp] or [DESC !/regex/ A.esp]
#[derive(Debug, Serialize, Deserialize)]
pub struct DESC {
    pub expression: Atomic,
    pub regex: String,
    pub is_negated: bool,
}
impl DESC {
    #[must_use]
    pub const fn new(expression: Atomic, regex: String, is_negated: bool) -> Self {
        Self {
            expression,
            regex,
            is_negated,
        }
    }
}
impl TExpression for DESC {
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        // check the version
        if let Some(plugins) = wild_contains_data(items, &self.expression.item) {
            let mut results = vec![];
            for p in &plugins {
                if let Some(description) = &p.description
                    && let Ok(pattern) = regex::Regex::new(&self.regex)
                {
                    let description = &description.to_lowercase();
                    if self.is_negated {
                        if !pattern.is_match(description) {
                            results.push(p.name.clone());
                        }
                    } else if pattern.is_match(description) {
                        results.push(p.name.clone());
                    }
                }
            }
            if results.is_empty() {
                return None;
            }
            return Some(results);
        }
        None
    }
}
impl Clone for DESC {
    fn clone(&self) -> Self {
        Self {
            expression: self.expression.clone(),
            regex: self.regex.clone(),
            is_negated: self.is_negated,
        }
    }
}

impl Display for DESC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_negated {
            write!(f, "[DESC !/{}/ {}]", self.regex, self.expression.clone())
        } else {
            write!(f, "[DESC /{}/ {}]", self.regex, self.expression.clone())
        }
    }
}

////////////////////////////////////////////////////////////////////////
// SIZE

/// The Size predicate is a special predicate that matches the filesize of the plugin
/// [SIZE ### A.esp] or [SIZE !### A.esp]
#[derive(Debug, Serialize, Deserialize)]
pub struct SIZE {
    pub expression: Atomic,
    pub size: u64,
    pub is_negated: bool,
}
impl SIZE {
    #[must_use]
    pub const fn new(expression: Atomic, size: u64, is_negated: bool) -> Self {
        Self {
            expression,
            size,
            is_negated,
        }
    }
}
impl TExpression for SIZE {
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        // check the size
        if let Some(plugins) = wild_contains_data(items, &self.expression.item) {
            let mut results = vec![];
            for p in &plugins {
                if self.is_negated {
                    if p.size != self.size {
                        results.push(p.name.clone());
                    }
                } else if p.size == self.size {
                    results.push(p.name.clone());
                }
            }
            if results.is_empty() {
                return None;
            }
            return Some(results);
        }
        None
    }
}
impl Clone for SIZE {
    fn clone(&self) -> Self {
        Self {
            expression: self.expression.clone(),
            size: self.size,
            is_negated: self.is_negated,
        }
    }
}

impl Display for SIZE {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_negated {
            write!(f, "[SIZE !{} {}]", self.size, self.expression.clone())
        } else {
            write!(f, "[SIZE {} {}]", self.size, self.expression.clone())
        }
    }
}

////////////////////////////////////////////////////////////////////////
// VER

/// Compares `actual` against `version` with `op` (`<`, `=` or `>`).
/// Invalid rule versions never match (the parser validates them beforehand).
fn version_matches(op: char, version: &str, actual: &semver::Version) -> bool {
    let Ok(version) = semver::Version::parse(version) else {
        log::warn!("Invalid version in rule: {version}");
        return false;
    };
    VersionReq::parse(&format!("{op}{version}")).is_ok_and(|req| req.matches(actual))
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum EVerOperator {
    Less,
    Equal,
    Greater,
}

impl Display for EVerOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Less => write!(f, "<"),
            Self::Equal => write!(f, "="),
            Self::Greater => write!(f, ">"),
        }
    }
}

/// The Ver predicate compares a plugin version.
///
/// It first tries to match the version number string stored in the plugin header,
/// and if that fails it tries to match the version number from the plugin filename.
/// If a version number is found, it can be used in a comparison.
/// Syntax: [VER operator version plugin.esp]
#[derive(Debug, Serialize, Deserialize)]
pub struct VER {
    pub expression: Atomic,
    pub operator: EVerOperator,
    pub version: String,
}
impl VER {
    #[must_use]
    pub const fn new(expression: Atomic, operator: EVerOperator, version: String) -> Self {
        Self {
            expression,
            operator,
            version,
        }
    }
}
impl TExpression for VER {
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        // check the version
        if let Some(plugins) = wild_contains_data(items, &self.expression.item) {
            let mut results = vec![];
            for p in &plugins {
                if let Some(plugin_version) = &p.version {
                    let op = match self.operator {
                        EVerOperator::Less => '<',
                        EVerOperator::Equal => '=',
                        EVerOperator::Greater => '>',
                    };
                    if version_matches(op, &self.version, plugin_version) {
                        results.push(p.name.clone());
                    }
                }
            }
            if results.is_empty() {
                return None;
            }
            return Some(results);
        }
        None
    }
}
impl Clone for VER {
    fn clone(&self) -> Self {
        Self {
            expression: self.expression.clone(),
            operator: self.operator,
            version: self.version.clone(),
        }
    }
}

impl Display for VER {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[VER {} {} {}]",
            self.operator,
            self.version,
            self.expression.clone()
        )
    }
}

////////////////////////////////////////////////////////////////////////
// GVER

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum EGVerOperator {
    Less,
    Equal,
    Greater,
}

impl Display for EGVerOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Less => write!(f, "<"),
            Self::Equal => write!(f, "="),
            Self::Greater => write!(f, ">"),
        }
    }
}

/// The GVER predicate is a special predicate that matches against the game version
/// If a version number is found, it can be used in a comparison.
/// Syntax: [GVER operator version plugin.esp]
#[derive(Debug, Serialize, Deserialize)]
pub struct GVER {
    pub expression: Atomic,
    pub operator: EGVerOperator,
    pub version: String,
}
impl GVER {
    #[must_use]
    pub const fn new(expression: Atomic, operator: EGVerOperator, version: String) -> Self {
        Self {
            expression,
            operator,
            version,
        }
    }
}
impl TExpression for GVER {
    fn eval(&self, items: &[PluginData]) -> Option<Vec<String>> {
        // check the version
        if let Some(plugins) = wild_contains_data(items, &self.expression.item) {
            let mut results = vec![];
            for p in &plugins {
                if let Some(game_version) = &p.game_version {
                    let op = match self.operator {
                        EGVerOperator::Less => '<',
                        EGVerOperator::Equal => '=',
                        EGVerOperator::Greater => '>',
                    };
                    if version_matches(op, &self.version, game_version) {
                        results.push(p.name.clone());
                    }
                }
            }
            if results.is_empty() {
                return None;
            }
            return Some(results);
        }
        None
    }
}
impl Clone for GVER {
    fn clone(&self) -> Self {
        Self {
            expression: self.expression.clone(),
            operator: self.operator,
            version: self.version.clone(),
        }
    }
}

impl Display for GVER {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[GVER {} {} {}]",
            self.operator,
            self.version,
            self.expression.clone()
        )
    }
}
