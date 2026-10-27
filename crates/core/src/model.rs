use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThreadKey {
    pub account: String,
    pub id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartBody {
    #[serde(default)]
    pub size: u64,
    pub data: Option<std::sync::Arc<str>>,
    pub attachment_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(default)]
    pub part_id: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub headers: Vec<Header>,
    #[serde(default)]
    pub body: PartBody,
    #[serde(default)]
    pub parts: Vec<Part>,
}

impl Part {
    pub fn header(&self, name: &str) -> &str {
        self.headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str())
            .unwrap_or("")
    }
    pub fn attachments(&self) -> Vec<&Part> {
        let mut out = Vec::new();
        self.walk(&mut |p| {
            if !p.filename.is_empty()
                || p.header("Content-Disposition")
                    .to_ascii_lowercase()
                    .starts_with("attachment")
                || (p.mime_type.starts_with("image/") && !p.header("Content-ID").is_empty())
            {
                out.push(p);
            }
        });
        out
    }
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Part)) {
        // Google has already parsed MIME. Cap recursive traversal of untrusted data.
        fn visit<'a>(p: &'a Part, f: &mut impl FnMut(&'a Part), depth: usize) {
            if depth > 32 {
                return;
            }
            f(p);
            for child in &p.parts {
                visit(child, f, depth + 1);
            }
        }
        visit(self, f, 0);
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    #[serde(default)]
    pub thread_id: String,
    #[serde(default)]
    pub label_ids: Vec<String>,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub internal_date: String,
    #[serde(default)]
    pub payload: Part,
    #[serde(default)]
    pub history_id: String,
}

impl Message {
    pub fn date(&self) -> i64 {
        self.internal_date.parse().unwrap_or(0)
    }
    pub fn has(&self, label: &str) -> bool {
        self.label_ids.iter().any(|l| l == label)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: String,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub key: ThreadKey,
    pub subject: String,
    pub sender: String,
    pub snippet: String,
    pub date: i64,
    pub unread: bool,
    pub starred: bool,
    pub important: bool,
    pub attachments: bool,
    pub count: usize,
}

impl Conversation {
    pub fn from_thread(account: &str, thread: &Thread) -> Option<Self> {
        let latest = thread.messages.iter().max_by_key(|m| m.date())?;
        Some(Self {
            key: ThreadKey {
                account: account.into(),
                id: thread.id.clone(),
            },
            subject: latest.payload.header("Subject").into(),
            sender: latest.payload.header("From").into(),
            snippet: latest.snippet.clone(),
            date: latest.date(),
            unread: thread.messages.iter().any(|m| m.has("UNREAD")),
            starred: thread.messages.iter().any(|m| m.has("STARRED")),
            important: thread.messages.iter().any(|m| m.has("IMPORTANT")),
            attachments: thread
                .messages
                .iter()
                .any(|m| !m.payload.attachments().is_empty()),
            count: thread.messages.len(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Folder {
    Inbox,
    AllMail,
    Unread,
    Starred,
    Sent,
    Drafts,
    Archive,
    Trash,
    Spam,
    Label(String),
}

impl Folder {
    pub fn matches(&self, thread: &Thread) -> bool {
        thread.messages.iter().any(|message| {
            if matches!(self, Self::Trash) {
                return message.has("TRASH");
            }
            if matches!(self, Self::Spam) {
                return message.has("SPAM");
            }
            if message.has("TRASH") || message.has("SPAM") {
                return false;
            }
            match self {
                Self::Inbox => message.has("INBOX"),
                Self::AllMail => true,
                Self::Unread => message.has("UNREAD"),
                Self::Starred => message.has("STARRED"),
                Self::Sent => message.has("SENT"),
                Self::Drafts => message.has("DRAFT"),
                Self::Archive => {
                    !message.has("INBOX") && !message.has("SENT") && !message.has("DRAFT")
                }
                Self::Label(id) => message.has(id),
                Self::Trash | Self::Spam => unreachable!(),
            }
        })
    }
    pub fn query(&self) -> String {
        match self {
            Self::Inbox => "in:inbox",
            Self::AllMail => "-in:spam -in:trash",
            Self::Unread => "is:unread -in:spam -in:trash",
            Self::Starred => "is:starred -in:spam -in:trash",
            Self::Sent => "in:sent",
            Self::Drafts => "in:drafts",
            Self::Archive => "-in:inbox -in:spam -in:trash -in:sent -in:drafts",
            Self::Trash => "in:trash",
            Self::Spam => "in:spam",
            Self::Label(_) => "-in:spam -in:trash",
        }
        .into()
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub messages_unread: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailAction {
    Read,
    Unread,
    Star,
    Unstar,
    Archive,
    Inbox,
    Trash,
    Restore,
    Spam,
    NotSpam,
    Important,
    Unimportant,
    AddLabel(String),
    RemoveLabel(String),
}

impl MailAction {
    /// Invert only labels touched by this operation, using each message's
    /// previous state rather than a conversation-wide guessed inverse.
    pub fn undo_for(&self, before: &Message) -> Vec<Self> {
        if matches!(self, Self::Trash) {
            return if before.has("TRASH") {
                vec![]
            } else {
                let mut actions = vec![Self::Restore];
                if before.has("INBOX") {
                    actions.push(Self::Inbox);
                }
                actions
            };
        }
        if matches!(self, Self::Restore) {
            return if before.has("TRASH") {
                vec![Self::Trash]
            } else {
                vec![]
            };
        }
        let (add, remove) = self.labels();
        add.into_iter()
            .filter(|id| !before.has(id))
            .map(Self::RemoveLabel)
            .chain(
                remove
                    .into_iter()
                    .filter(|id| before.has(id))
                    .map(Self::AddLabel),
            )
            .collect()
    }
    pub fn labels(&self) -> (Vec<String>, Vec<String>) {
        let (add, remove) = match self {
            Self::Read => (None, Some("UNREAD")),
            Self::Unread => (Some("UNREAD"), None),
            Self::Star => (Some("STARRED"), None),
            Self::Unstar => (None, Some("STARRED")),
            Self::Archive => (None, Some("INBOX")),
            Self::Inbox => (Some("INBOX"), None),
            Self::Important => (Some("IMPORTANT"), None),
            Self::Unimportant => (None, Some("IMPORTANT")),
            Self::Spam => (Some("SPAM"), Some("INBOX")),
            Self::NotSpam => (Some("INBOX"), Some("SPAM")),
            Self::AddLabel(id) => (Some(id.as_str()), None),
            Self::RemoveLabel(id) => (None, Some(id.as_str())),
            Self::Trash | Self::Restore => (None, None),
        };
        (
            add.into_iter().map(String::from).collect(),
            remove.into_iter().map(String::from).collect(),
        )
    }
}
