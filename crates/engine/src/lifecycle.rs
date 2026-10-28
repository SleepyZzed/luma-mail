//! Exit waits for accepted writes; read-only work can be interrupted.
pub fn finish_before_exit(kind: &str, id: u64, auth_commit: u64) -> bool {
    if kind == "authStart" {
        return id != 0 && id == auth_commit;
    }
    kind.starts_with("compose")
        || matches!(
            kind,
            "mutate"
                | "markRead"
                | "undo"
                | "draftDelete"
                | "senderRule"
                | "labels"
                | "settingsUpdate"
                | "accountUpdate"
                | "accountReorder"
                | "accountRemove"
                | "signatureSave"
                | "signatureUploadPicture"
                | "attachmentSave"
                | "attachmentSaveAll"
        )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writes_are_finished_but_reads_and_uncommitted_signin_can_stop() {
        for kind in [
            "mutate",
            "markRead",
            "undo",
            "draftDelete",
            "senderRule",
            "labels",
            "settingsUpdate",
            "accountRemove",
            "signatureSave",
            "signatureUploadPicture",
            "composeSend",
            "composeCloseAnswer",
            "composeAttach",
            "attachmentSaveAll",
        ] {
            assert!(finish_before_exit(kind, 4, 0), "{kind}")
        }
        for kind in [
            "view",
            "page",
            "select",
            "message",
            "threadPage",
            "renderMessage",
            "photoRequest",
            "authStart",
        ] {
            assert!(!finish_before_exit(kind, 4, 0), "{kind}")
        }
        assert!(finish_before_exit("authStart", 4, 4));
        assert!(!finish_before_exit("authStart", 3, 4));
        assert!(!finish_before_exit("authStart", 0, 0));
    }
}
