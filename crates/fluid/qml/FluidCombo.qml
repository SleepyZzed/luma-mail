import QtQuick
import QtQuick.Controls.Basic
ComboBox {
    id:control
    implicitHeight:Theme.unit(32);leftPadding:Theme.unit(9);rightPadding:Theme.unit(25)
    font.pixelSize:Theme.unit(11)
    contentItem:Text{textFormat:Text.PlainText;text:control.displayText;color:control.enabled?Theme.text:Theme.faint;font:control.font;verticalAlignment:Text.AlignVCenter;elide:Text.ElideRight}
    background:Rectangle{color:control.hovered?Theme.hover:Theme.surface;radius:Theme.unit(8);border.width:control.activeFocus?1:0;border.color:Theme.accent}
    indicator:Icon{name:"chevron-down";width:Theme.unit(12);height:width;x:control.width-width-Theme.unit(7);anchors.verticalCenter:parent.verticalCenter;color:Theme.muted;visible:control.count>1}
    popup:Popup{
        popupType:Popup.Item;transformOrigin:Item.TopLeft
        enter:PopupEnter{}
        exit:PopupExit{}
        y:control.height+Theme.unit(5);width:Math.max(control.width,Theme.unit(190));padding:Theme.unit(5)
        implicitHeight:Math.min(Theme.unit(300),list.contentHeight+2*padding)
        background:Rectangle{radius:Theme.unit(10);color:Theme.raised;border.width:1;border.color:Theme.outline}
        contentItem:ListView{id:list;clip:true;implicitHeight:contentHeight;model:control.popup.visible?control.delegateModel:null;currentIndex:control.highlightedIndex;ScrollBar.vertical:ScrollBar{implicitWidth:4}}
    }
    delegate:ItemDelegate{
        required property int index
        required property var modelData
        width:ListView.view.width;height:Theme.unit(31);highlighted:control.highlightedIndex===index
        contentItem:Text{textFormat:Text.PlainText;text:control.textRole?modelData[control.textRole]:String(modelData);color:Theme.text;font:control.font;elide:Text.ElideRight;verticalAlignment:Text.AlignVCenter}
        background:Rectangle{radius:Theme.unit(6);color:parent.highlighted?Theme.selected:Theme.raised}
    }
}
