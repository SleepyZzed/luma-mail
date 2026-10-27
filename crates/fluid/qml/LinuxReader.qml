import QtQuick
import QtWebEngine
HtmlSurface {
    id:root
    property string page:""
    property bool paused:false
    property var masks:[]
    property real uiScale:1
    property real measuredHeight:48
    property string htmlError:""
    implicitHeight:measuredHeight
    function updatePage(){if(page)browser.loadHtml(page,"about:blank")}
    onPageChanged:updatePage()
    Component.onCompleted:updatePage()
    function measure(){browser.runJavaScript("Math.ceil(document.body?document.body.getBoundingClientRect().height:48)",WebEngineScript.ApplicationWorld,value=>{if(isFinite(value))root.measuredHeight=Math.max(24,Math.min(60000,value*root.uiScale))})}
    function scrollDocument(){browser.runJavaScript("window.scrollTo(0,"+(root.viewportTop/root.uiScale)+")",WebEngineScript.ApplicationWorld)}
    onViewportChanged:scrollDocument()
    Timer{id:resizeDelay;interval:80;onTriggered:root.measure()}
    onWidthChanged:resizeDelay.restart()
    onUiScaleChanged:resizeDelay.restart()
    WebEngineView {
        id:browser;width:parent.width;y:root.viewportTop;height:Math.max(1,root.viewportHeight);profile:ReaderProfile;zoomFactor:root.uiScale;backgroundColor:"transparent";audioMuted:true
        settings.javascriptEnabled:false;settings.localStorageEnabled:false;settings.localContentCanAccessFileUrls:false;settings.localContentCanAccessRemoteUrls:false
        settings.pluginsEnabled:false;settings.pdfViewerEnabled:false;settings.hyperlinkAuditingEnabled:false;settings.errorPageEnabled:false;settings.screenCaptureEnabled:false
        settings.autoLoadImages:true;settings.dnsPrefetchEnabled:false
        onLoadingChanged:info=>{if(info.status===WebEngineView.LoadSucceededStatus){root.measure();root.scrollDocument();root.loaded()}else if(info.status===WebEngineView.LoadFailedStatus)root.htmlError="HTML preview could not load. Reopen the message to retry."}
        onContentsSizeChanged:resizeDelay.restart()
        onNavigationRequested:request=>{if(request.url.toString()==="about:blank"||request.url.toString().startsWith("data:text/html"))request.action=WebEngineNavigationRequest.AcceptRequest;else{request.action=WebEngineNavigationRequest.IgnoreRequest;if(request.isMainFrame)Mail.openLink(request.url.toString())}}
        onNewWindowRequested:request=>Mail.openLink(request.requestedUrl.toString())
        onPermissionRequested:permission=>permission.deny()
        onFullScreenRequested:request=>request.reject()
        onContextMenuRequested:request=>request.accepted=true
        onFileDialogRequested:request=>{request.accepted=true;request.dialogReject()}
        onCertificateError:request=>request.rejectCertificate()
        onRenderProcessTerminated:{root.htmlError="The HTML renderer stopped. Reopen the message to retry."}
        WheelHandler{target:null;onWheel:event=>{root.wheel(event.angleDelta.y!==0?-event.angleDelta.y/3:-event.pixelDelta.y);event.accepted=true}}
    }
    Connections{target:ReaderProfile;function onDownloadRequested(download){download.cancel()}}
    Text{textFormat:Text.PlainText;anchors.fill:parent;visible:root.htmlError!=="";text:root.htmlError;wrapMode:Text.Wrap;color:Theme.muted;font.pixelSize:Theme.unit(12)}
}
