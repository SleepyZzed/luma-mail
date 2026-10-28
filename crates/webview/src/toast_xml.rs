pub fn document(title: &str, body: &str, copy_code: bool) -> String {
    fn escape(value: &str, limit: usize) -> String {
        value
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(limit)
            .collect::<String>()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
    format!(
        "<toast launch='open'><visual><binding template='ToastGeneric'><text>{}</text><text>{}</text></binding></visual><actions><action content='Open' arguments='open' activationType='foreground'/>{}</actions></toast>",
        escape(title, 200),
        escape(body, 600),
        if copy_code {
            "<action content='Copy code' arguments='copy' activationType='foreground'/>"
        } else {
            ""
        }
    )
}
#[cfg(test)]
mod tests {
    #[test]
    fn sender_markup_cannot_inject_actions() {
        let xml = super::document("<action arguments='evil'/>", "A&B", true);
        assert!(xml.contains("&lt;action"));
        assert!(xml.contains("A&amp;B"));
        assert_eq!(xml.matches("<action ").count(), 2);
        assert!(!super::document("mail", "body", false).contains("Copy code"));
    }
}
