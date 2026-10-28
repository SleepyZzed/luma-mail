import QtQuick
import QtQuick.Controls
TextField {
    id:root
    Accessible.name:placeholderText
    implicitHeight:Theme.unit(38)
    padding:Theme.unit(11);selectByMouse:true
    font.family:Theme.family;font.pixelSize:Theme.unit(12)
    color:Theme.text;placeholderTextColor:Theme.faint;selectionColor:Theme.selected;selectedTextColor:Theme.text
    background:Rectangle{radius:Theme.unit(10);color:Theme.window;border.width:1;border.color:root.activeFocus?Theme.accent:Theme.outline}
}
