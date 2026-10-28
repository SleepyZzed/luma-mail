pub fn document(title: &str, body: &str, copy_code: bool, activation: &str) -> Option<String> {
    if activation.len() != 36
        || !activation.bytes().enumerate().all(|(at, c)| {
            if [8, 13, 18, 23].contains(&at) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
    {
        return None;
    }
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
    Some(format!(
        "<toast launch='open:{activation}'><visual><binding template='ToastGeneric'><text>{}</text><text>{}</text></binding></visual><actions><action content='Open' arguments='open:{activation}' activationType='foreground'/>{}</actions></toast>",
        escape(title, 200),
        escape(body, 600),
        if copy_code {
            format!(
                "<action content='Copy code' arguments='copy:{activation}' activationType='foreground'/>"
            )
        } else {
            String::new()
        }
    ))
}
#[cfg(test)]
mod tests {
    #[test]
    fn sender_markup_cannot_inject_actions() {
        let id = "aad94b28-e682-4f13-a026-da0e286738f2";
        let xml = super::document("<action arguments='evil'/>", "A&B", true, id).unwrap();
        assert!(xml.contains("&lt;action"));
        assert!(xml.contains("A&amp;B"));
        assert_eq!(xml.matches("<action ").count(), 2);
        assert!(
            !super::document("mail", "body", false, id)
                .unwrap()
                .contains("Copy code")
        );
        assert!(super::document("mail", "body", true, "bad' /><action/>").is_none());
    }
}
