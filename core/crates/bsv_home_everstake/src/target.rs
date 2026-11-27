use bsv_home_client::Target;

#[derive(Clone, Debug)]
pub enum EverstakeTarget {
    GetStats,
}

impl Target for EverstakeTarget {
    fn path(&self) -> String {
        match self {
            Self::GetStats => "/api/v1/stats".to_string(),
        }
    }
}
