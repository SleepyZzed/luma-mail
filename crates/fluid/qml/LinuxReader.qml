import QtQuick
import QtWebEngine
HtmlSurface {
    id:root
    property string page:""
    property bool paused:false
    property var masks:[]
    property real uiScale:1
    property real revealProgress:1
    property bool viewReady:false
    readonly property string viewError:htmlError
    property int pageRevision:0
    property real lastScroll:-1
    property real measuredHeight:48
    property string htmlError:""
    property var privateProfile:ReaderProfiles.acquire()
    implicitHeight:measuredHeight
    function updatePage(){++pageRevision;viewReady=false;htmlError="";lastScroll=-1;if(page)browser.loadHtml(page,"about:blank#luma-"+pageRevision)}
    onPageChanged:updatePage()
    Component.onCompleted:updatePage()
    Component.onDestruction:ReaderProfiles.release()
    function measure(present=false){const revision=pageRevision;browser.runJavaScript("Math.ceil(document.body?document.body.getBoundingClientRect().height:48)",WebEngineScript.ApplicationWorld,value=>{if(revision!==root.pageRevision||!isFinite(value))return;root.measuredHeight=Math.max(24,Math.min(60000,value*root.uiScale));if(present){root.scrollDocument(true);root.viewReady=true;root.loaded()}})}
    function scrollDocument(force=false){const offset=root.viewportTop/root.uiScale;if(!force&&(!viewReady||offset===lastScroll))return;lastScroll=offset;browser.runJavaScript("window.scrollTo(0,"+offset+")",WebEngineScript.ApplicationWorld)}
    onViewportChanged:scrollDocument()
    Timer{id:resizeDelay;interval:80;onTriggered:root.measure()}
    onWidthChanged:resizeDelay.restart()
    onUiScaleChanged:resizeDelay.restart()
    Item{id:revealClip;x:0;y:root.viewportTop;width:Math.min(root.width,Theme.unit(260))*(1-root.revealProgress)+root.width*root.revealProgress;height:Math.max(0,Math.min(root.viewportHeight,Theme.unit(38))*(1-root.revealProgress)+root.viewportHeight*root.revealProgress);clip:true
    WebEngineView {
        id:browser;width:root.width;height:Math.max(1,root.viewportHeight);opacity:root.viewReady?1:0;profile:root.privateProfile;zoomFactor:root.uiScale;backgroundColor:Theme.window;audioMuted:true
        settings.javascriptEnabled:false;settings.localStorageEnabled:false;settings.localContentCanAccessFileUrls:false;settings.localContentCanAccessRemoteUrls:false
        settings.pluginsEnabled:false;settings.pdfViewerEnabled:false;settings.hyperlinkAuditingEnabled:false;settings.errorPageEnabled:false;settings.screenCaptureEnabled:false
        settings.autoLoadImages:true;settings.dnsPrefetchEnabled:false
        onLoadingChanged:info=>{if(info.url.toString()!=="about:blank#luma-"+root.pageRevision)return;if(info.status===WebEngineView.LoadSucceededStatus)root.measure(true);else if(info.status===WebEngineView.LoadFailedStatus){root.viewReady=false;root.htmlError="HTML preview could not load. Reopen the message to retry."}}
        onContentsSizeChanged:resizeDelay.restart()
        onNavigationRequested:request=>{if(request.url.toString()==="about:blank"||request.url.toString().startsWith("about:blank#luma-")||request.url.toString().startsWith("data:text/html"))request.action=WebEngineNavigationRequest.AcceptRequest;else{request.action=WebEngineNavigationRequest.IgnoreRequest;if(request.isMainFrame)Mail.openLink(request.url.toString())}}
        onNewWindowRequested:request=>Mail.openLink(request.requestedUrl.toString())
        onPermissionRequested:permission=>permission.deny()
        onFullScreenRequested:request=>request.reject()
        onContextMenuRequested:request=>request.accepted=true
        onFileDialogRequested:request=>{request.accepted=true;request.dialogReject()}
        onCertificateError:request=>request.rejectCertificate()
        onRenderProcessTerminated:{root.viewReady=false;root.htmlError="The HTML renderer stopped. Reopen the message to retry."}
        WheelHandler{target:null;onWheel:event=>{root.wheel(event.angleDelta.y!==0?-event.angleDelta.y/3:-event.pixelDelta.y);event.accepted=true}}
    }
    }
    Connections{target:root.privateProfile;function onDownloadRequested(download){download.cancel()}}
    Text{textFormat:Text.PlainText;anchors.fill:parent;visible:root.htmlError!=="";text:root.htmlError;wrapMode:Text.Wrap;color:Theme.muted;font.pixelSize:Theme.unit(12)}
}
