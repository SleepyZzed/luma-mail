use crate::{Result, model::*};
use async_trait::async_trait;

pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
}

#[async_trait]
pub trait MailProvider: Send + Sync {
    fn account_id(&self) -> &str;
    async fn list(
        &self,
        folder: &Folder,
        search: &str,
        page: Option<&str>,
    ) -> Result<Page<Conversation>>;
    async fn thread(&self, id: &str) -> Result<Thread>;
    async fn message(&self, id: &str) -> Result<Message>;
    async fn act(&self, id: &str, message: bool, action: &MailAction) -> Result<()>;
    async fn labels(&self) -> Result<Vec<Label>>;
}
