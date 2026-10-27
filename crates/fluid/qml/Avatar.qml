import QtQuick
Rectangle {
    id: root
    property string sender: ""
    property string photo: ""
    implicitWidth: Theme.unit(40); implicitHeight: implicitWidth;radius: width/2
    color: Theme.avatarColor(sender)
    Text {textFormat:Text.PlainText;anchors.centerIn: parent;text:Theme.initials(root.sender);font.family:Theme.family;font.pixelSize:root.width*.31;font.weight:Font.DemiBold;color:"#fff";visible:root.photo==""}
    Image {anchors.fill:parent;source:root.photo;fillMode:Image.PreserveAspectCrop;visible:source!=""}
}
