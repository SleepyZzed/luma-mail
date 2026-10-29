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
    readonly property bool renderActive:windowRenderable&&!paused
    readonly property bool documentFrozen:browser.lifecycleState===WebEngineView.LifecycleState.Frozen
    property bool pagePending:false
    property bool needsMeasurement:true
    property int documentLoads:0
    property var privateProfile:ReaderProfiles.acquire()
    implicitHeight:measuredHeight
    function loadPendingPage(){if(!pagePending||!renderActive)return;pagePending=false;browser.lifecycleState=WebEngineView.LifecycleState.Active;++documentLoads;browser.loadHtml(page,"about:blank#luma-"+pageRevision)}
    function updatePage(){++pageRevision;viewReady=false;htmlError="";lastScroll=-1;needsMeasurement=true;pagePending=page!=="";loadPendingPage()}
    onPageChanged:updatePage()
    Component.onCompleted:updatePage()
    Component.onDestruction:ReaderProfiles.release()
    function measure(present=false){needsMeasurement=true;if(!renderActive||browser.loading||pagePending)return;const revision=pageRevision;browser.runJavaScript("Math.ceil(document.body?document.body.getBoundingClientRect().height:48)",WebEngineScript.ApplicationWorld,value=>{if(revision!==root.pageRevision||!root.renderActive||!isFinite(value))return;root.needsMeasurement=false;root.measuredHeight=Math.max(24,Math.min(60000,value*root.uiScale));if(present){root.scrollDocument(true);root.viewReady=true;root.loaded()}})}
    function scrollDocument(force=false){if(!renderActive)return;const offset=root.viewportTop/root.uiScale;if(!force&&(!viewReady||offset===lastScroll))return;lastScroll=offset;browser.runJavaScript("window.scrollTo(0,"+offset+")",WebEngineScript.ApplicationWorld)}
    function updateLifecycle(){
        if(renderActive){browser.lifecycleState=WebEngineView.LifecycleState.Active;loadPendingPage();if(htmlError===""){if(needsMeasurement||!viewReady)resizeDelay.restart();else scrollDocument(true)}}
        else {resizeDelay.stop();if(!browser.loading)browser.lifecycleState=WebEngineView.LifecycleState.Frozen}
    }
    onRenderActiveChanged:{resizeDelay.stop();Qt.callLater(updateLifecycle)}
    onViewportChanged:scrollDocument()
    Timer{id:resizeDelay;interval:80;onTriggered:root.measure(!root.viewReady)}
    function scheduleMeasurement(){needsMeasurement=true;if(renderActive)resizeDelay.restart()}
    onWidthChanged:scheduleMeasurement()
    onUiScaleChanged:scheduleMeasurement()
    Item{id:revealClip;x:0;y:root.viewportTop;width:Math.min(root.width,Theme.unit(260))*(1-root.revealProgress)+root.width*root.revealProgress;height:Math.max(0,Math.min(root.viewportHeight,Theme.unit(38))*(1-root.revealProgress)+root.viewportHeight*root.revealProgress);clip:true
    WebEngineView {
        id:browser;width:root.width;height:Math.max(1,root.viewportHeight);visible:root.renderActive;opacity:root.viewReady?1:0;profile:root.privateProfile;zoomFactor:root.uiScale;backgroundColor:Theme.window;audioMuted:true
        settings.javascriptEnabled:false;settings.localStorageEnabled:false;settings.localContentCanAccessFileUrls:false;settings.localContentCanAccessRemoteUrls:false
        settings.pluginsEnabled:false;settings.pdfViewerEnabled:false;settings.hyperlinkAuditingEnabled:false;settings.errorPageEnabled:false;settings.screenCaptureEnabled:false
        settings.autoLoadImages:true;settings.dnsPrefetchEnabled:false
        onLoadingChanged:info=>{Qt.callLater(root.updateLifecycle);if(info.url.toString()!=="about:blank#luma-"+root.pageRevision)return;if(info.status===WebEngineView.LoadSucceededStatus)root.needsMeasurement=true;else if(info.status===WebEngineView.LoadFailedStatus){root.viewReady=false;root.htmlError="HTML preview could not load. Reopen the message to retry."}}
        onContentsSizeChanged:root.scheduleMeasurement()
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
