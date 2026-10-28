import QtQuick
FluidMenu {
    id:menu
    property var editor
    property var richDocument:null
    property var pasteAction:null
    readonly property bool selected:!!editor&&editor.selectionStart!==editor.selectionEnd
    readonly property bool writable:!!editor&&!editor.readOnly
    function paste(){if(pasteAction)pasteAction();else editor.paste()}
    FluidMenuItem{text:"Undo";glyph:"undo";enabled:menu.writable&&(menu.richDocument?menu.richDocument.canUndo:menu.editor?.canUndo||false);onTriggered:menu.richDocument?menu.richDocument.undo():menu.editor.undo()}
    FluidMenuItem{text:"Redo";glyph:"redo";enabled:menu.writable&&(menu.richDocument?menu.richDocument.canRedo:menu.editor?.canRedo||false);onTriggered:menu.richDocument?menu.richDocument.redo():menu.editor.redo()}
    FluidMenuSeparator{}
    FluidMenuItem{text:"Cut";enabled:menu.writable&&menu.selected;onTriggered:menu.editor.cut()}
    FluidMenuItem{text:"Copy";enabled:menu.selected;onTriggered:menu.editor.copy()}
    FluidMenuItem{objectName:"pasteEditAction";text:"Paste";enabled:menu.writable&&(menu.pasteAction||menu.editor?.canPaste||false);onTriggered:menu.paste()}
    FluidMenuItem{text:"Delete";glyph:"trash";enabled:menu.writable&&menu.selected;onTriggered:menu.editor.remove(menu.editor.selectionStart,menu.editor.selectionEnd)}
    FluidMenuSeparator{}
    FluidMenuItem{text:"Select all";glyph:"select";enabled:!!menu.editor&&menu.editor.length>0;onTriggered:menu.editor.selectAll()}
}
