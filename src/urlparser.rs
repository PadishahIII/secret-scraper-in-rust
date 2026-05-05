use url::Url;

#[derive(Debug)]
pub struct URLNode {
    url: String,
    url_obj: Url,
    response_status: u16,
    depth: u32,
}
impl URLNode {
    pub fn new(url: &str) -> Result<Self> {
        let url_obj = Url::parse(&url)?;
        url_obj.to_string()
    }
}
