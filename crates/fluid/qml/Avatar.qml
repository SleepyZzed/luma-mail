import QtQuick
Rectangle {
    id:root
    property string sender:""
    property string photo:""
    property string account:""
    property string photoSender:sender
    property bool active:true
    property bool ready:false
    property string watchedKey:""
    property string profileSource:""
    implicitWidth:Theme.unit(40);implicitHeight:implicitWidth;radius:width/2;color:Theme.avatarColor(sender)
    function subscribe(){
        if(!ready)return
        const wanted=active&&visible&&account&&photoSender&&!photo?account+"\u0000"+photoSender:""
        if(wanted&&wanted===watchedKey){profileSource=Mail.profileSource(watchedKey);return}
        if(watchedKey)Mail.releaseProfile(watchedKey)
        watchedKey="";profileSource=""
        if(active&&visible&&account&&photoSender&&!photo){watchedKey=Mail.watchProfile(account,photoSender);profileSource=Mail.profileSource(watchedKey)}
    }
    onAccountChanged:if(ready)Qt.callLater(subscribe)
    onPhotoSenderChanged:if(ready)Qt.callLater(subscribe)
    onActiveChanged:if(ready)Qt.callLater(subscribe)
    onVisibleChanged:if(ready)Qt.callLater(subscribe)
    onPhotoChanged:if(ready)Qt.callLater(subscribe)
    Component.onCompleted:{ready=true;subscribe()}
    Component.onDestruction:{ready=false;if(watchedKey)Mail.releaseProfile(watchedKey)}
    Text{textFormat:Text.PlainText;anchors.centerIn:parent;text:Theme.initials(root.sender);font.family:Theme.family;font.pixelSize:root.width*.31;font.weight:Font.DemiBold;color:"#fff";visible:reveal.value<.99}
    Image{id:picture;objectName:root.objectName+"Picture";anchors.centerIn:parent;width:root.width*Math.max(0,Math.min(1,reveal.value));height:width;source:root.photo||root.profileSource;sourceSize:Qt.size(96,96);cache:false;asynchronous:true;fillMode:Image.PreserveAspectCrop}
    Motion{id:reveal;target:picture.status===Image.Ready&&picture.source.toString()!==""?1:0;enabled:Theme.motion;epsilon:.002}
    Connections{target:Mail
        function onProfileChanged(key){if(key===root.watchedKey)root.profileSource=Mail.profileSource(key)}
        function onProfilesReset(){root.watchedKey="";root.profileSource="";if(root.ready)Qt.callLater(root.subscribe)}
    }
}
