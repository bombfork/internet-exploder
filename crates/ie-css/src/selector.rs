#[derive(Debug, Clone, PartialEq)]
pub enum Selector {
    Simple(SimpleSelector),
    Compound(Vec<SimpleSelector>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimpleSelector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
}
