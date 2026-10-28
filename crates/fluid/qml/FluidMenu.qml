import QtQuick
import QtQuick.Controls.Basic
Menu {
    id:root
    popupType:Popup.Item
    implicitWidth:Theme.unit(292)
    margins:Theme.unit(8)
    padding:Theme.unit(6)
    transformOrigin:Item.TopLeft
    enter:PopupEnter{}
    exit:PopupExit{}
    delegate:FluidMenuItem{}
    font.family:Theme.family;font.pixelSize:Theme.unit(12)
    palette.window:Theme.surface;palette.base:Theme.surface;palette.button:Theme.surface
    palette.text:Theme.text;palette.buttonText:Theme.text;palette.highlight:Theme.selected;palette.highlightedText:Theme.text
    background:Rectangle{radius:Theme.unit(13);color:Theme.surface;border.width:1;border.color:Theme.outline}
}
