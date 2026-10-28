import QtQuick
import QtQuick.Controls.Basic
Dialog {
    id:control
    popupType:Popup.Item
    padding:Theme.unit(22)
    margins:Theme.unit(16)
    font.family:Theme.family;font.pixelSize:Theme.unit(12)
    palette.window:Theme.surface;palette.base:Theme.window;palette.text:Theme.text
    palette.buttonText:Theme.text;palette.highlight:Theme.selected;palette.highlightedText:Theme.text
    transformOrigin:Item.Center
    enter:PopupEnter{}
    exit:PopupExit{}
    background:Rectangle{radius:Theme.unit(22);color:Theme.surface;border.width:1;border.color:Theme.outline}
    header:Text{textFormat:Text.PlainText;text:control.title;visible:text!=="";color:Theme.text;font.family:Theme.family;font.pixelSize:Theme.unit(19);font.weight:Font.DemiBold;wrapMode:Text.Wrap;padding:Theme.unit(22);bottomPadding:0}
    footer:DialogButtonBox {
        visible:control.standardButtons!==Dialog.NoButton
        padding:Theme.unit(16);spacing:Theme.unit(8)
        delegate:FluidButton{quiet:false}
        background:Item{}
    }
    Overlay.modal:Rectangle{color:Qt.rgba(Theme.window.r,Theme.window.g,Theme.window.b,.55)}
}
