import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
T.TextField {
    id:root
    property TextEditMenu editMenu:null
    Accessible.name:placeholderText
    implicitHeight:Theme.unit(38)
    padding:Theme.unit(11);selectByMouse:true
    font.family:Theme.family;font.pixelSize:Theme.unit(12)
    color:Theme.text;placeholderTextColor:Theme.faint;selectionColor:Theme.selected;selectedTextColor:Theme.text
    background:Rectangle{radius:Theme.unit(10);color:Theme.window;border.width:1;border.color:root.activeFocus?Theme.accent:Theme.outline}
    Text{textFormat:Text.PlainText;x:root.leftPadding;y:root.topPadding;width:root.width-root.leftPadding-root.rightPadding;height:root.height-root.topPadding-root.bottomPadding;verticalAlignment:Text.AlignVCenter;text:root.placeholderText;font:root.font;color:root.placeholderTextColor;visible:!root.length&&!root.preeditText;elide:Text.ElideRight}
    Component{id:menuFactory;TextEditMenu{onClosed:destroy()}}
    function openEditMenu(position){if(!editMenu)editMenu=menuFactory.createObject(root,{objectName:root.objectName+"ContextMenu",editor:root});editMenu.popup(root,position)}
    TapHandler{acceptedButtons:Qt.RightButton;onTapped:event=>root.openEditMenu(event.position)}
    Shortcut{sequences:["Shift+F10","Menu"];enabled:root.activeFocus;onActivated:root.openEditMenu(Qt.point(root.cursorRectangle.x,root.cursorRectangle.y+root.cursorRectangle.height))}
}
