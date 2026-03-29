//! Protocol backends: HTTP, Usenet, HLS, DASH.

pub mod dash;
pub mod hls;
pub mod http;
pub mod usenet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    Http,
    Hls,
    Dash,
    Usenet,
}
