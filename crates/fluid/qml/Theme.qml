pragma Singleton
import QtQuick
QtObject {
    id:theme
    readonly property var tokens:Mail.palette
    readonly property bool dark:tokens.mode!=="light"
    property FluidColor windowColor:FluidColor{target:theme.tokens.background||"#19171e"}
    property FluidColor surfaceColor:FluidColor{target:theme.tokens.surface||"#24212b"}
    property FluidColor textColor:FluidColor{target:theme.tokens.text||"#f4eff8"}
    property FluidColor accentColor:FluidColor{target:theme.tokens.accent||"#c6b0f5"}
    readonly property color window:windowColor.value
    readonly property color surface:surfaceColor.value
    readonly property color text:textColor.value
    readonly property color accent:accentColor.value
    readonly property color rail:mix(window,surface,.45)
    readonly property color raised:dark?Qt.lighter(surface,1.15):surface
    readonly property color hover:tokens.hover||"#302a3b"
    readonly property color outline:tokens.outline||"#393240"
    readonly property color muted:tokens.muted||"#a39aae"
    readonly property color faint:mix(muted,window,.25)
    readonly property color accentText:tokens.accentText||"#281c3d"
    readonly property color selected:mix(surface,accent,dark?.19:.12)
    readonly property color link: "#82b5ff"
    readonly property real scale: Math.max(.75,Math.min(2,(Mail.settings.ui_scale_percent || 100)/100))
    readonly property bool motion: Mail.windowVisible&&!(Mail.settings.reduced_motion || false)
    readonly property string family: Qt.application.font.family
    function mix(a,b,t){return Qt.rgba(a.r+(b.r-a.r)*t,a.g+(b.g-a.g)*t,a.b+(b.b-a.b)*t,a.a+(b.a-a.a)*t)}
    function unit(n) {return n*scale}
    function initials(sender) {let words=sender.replace(/<.*>/g,'').trim().split(/[\s@._-]+/);return (words[0]?.[0]||'?').toUpperCase()+(words.length>1?(words[1]?.[0]||'').toUpperCase():'')}
    function avatarColor(sender) {let h=0;for(let i=0;i<sender.length;++i)h=(h*31+sender.charCodeAt(i))%360;return Qt.hsla(h/360,.30,.53,1)}
    function date(timestamp) {let d=new Date(Number(timestamp));let now=new Date();return d.toDateString()===now.toDateString()?Qt.formatTime(d,"hh:mm"):Qt.formatDate(d,"d MMM")}
    function folderTitle(key) {if(key.startsWith("label:")){const labels=Mail.labelSets[Mail.account]||[];const label=labels.find(l=>l.id===key.substring(6));return label?label.name:"Label"}return ({inbox:"Inbox",all_mail:"All Mail",unread:"Unread",starred:"Starred",sent:"Sent",drafts:"Drafts",archive:"Archive",spam:"Spam",trash:"Trash"})[key]||"Label"}
}
