#include "html_surface.h"
#include <QQuickWindow>
#include <QLibrary>
#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPointer>
#include <QElapsedTimer>
#include <QFileInfo>
#include <cmath>
#ifdef Q_OS_WIN
namespace {
struct Api {
    QLibrary loader,library;using Callback=void(*)(void*,const unsigned char*,size_t);
    using Create=void*(*)(qintptr,const unsigned char*,size_t,const unsigned char*,size_t,Callback,void*);
    Create create=nullptr;void(*destroy)(void*)=nullptr;void(*document)(void*,const unsigned char*,size_t)=nullptr;
    void(*visible)(void*,bool)=nullptr;void(*geometry)(void*,const int*,size_t,double,double)=nullptr;void(*measure)(void*,int,quint64)=nullptr;
    void(*background)(void*,quint32)=nullptr;void(*reveal)(void*,double)=nullptr;
    Api(){auto path=qEnvironmentVariable("LUMA_WEBVIEW");if(path.isEmpty())path=QCoreApplication::applicationDirPath()+"/luma_webview.dll";
        loader.setFileName(QFileInfo(path).absolutePath()+"/WebView2Loader.dll");loader.load();library.setFileName(path);if(!library.load())return;
        create=reinterpret_cast<Create>(library.resolve("luma_webview_create"));destroy=reinterpret_cast<decltype(destroy)>(library.resolve("luma_webview_destroy"));document=reinterpret_cast<decltype(document)>(library.resolve("luma_webview_document"));visible=reinterpret_cast<decltype(visible)>(library.resolve("luma_webview_visible"));geometry=reinterpret_cast<decltype(geometry)>(library.resolve("luma_webview_geometry"));measure=reinterpret_cast<decltype(measure)>(library.resolve("luma_webview_measure"));background=reinterpret_cast<decltype(background)>(library.resolve("luma_webview_background"));reveal=reinterpret_cast<decltype(reveal)>(library.resolve("luma_webview_reveal"));}
    bool valid()const{return create&&destroy&&document&&visible&&geometry&&measure&&background&&reveal;}
};
Api&api(){static Api instance;return instance;}
}
#endif
namespace {int nativeSurfaceCount=0;}
int HtmlSurface::liveNativeSurfaces(){return nativeSurfaceCount;}
void HtmlSurface::setActive(bool value){if(lifetimeActive==value)return;lifetimeActive=value;emit activeChanged();updateSurface();}
HtmlSurface::HtmlSurface(QQuickItem*parent):QQuickItem(parent){
    connect(qApp,&QCoreApplication::aboutToQuit,this,[this]{release();runtime.reset();});
    debounce.setSingleShot(true);debounce.setInterval(80);connect(&debounce,&QTimer::timeout,this,&HtmlSurface::measure);
    connect(this,&QQuickItem::visibleChanged,this,&HtmlSurface::updateSurface);
    connect(this,&QQuickItem::windowChanged,this,[this]{release();ancestors();updateSurface();});
    connect(this,&QQuickItem::parentChanged,this,[this]{ancestors();updateSurface();});
}
HtmlSurface::~HtmlSurface(){release();}
void HtmlSurface::componentComplete(){QQuickItem::componentComplete();complete=true;ancestors();updateSurface();}
void HtmlSurface::geometryChange(const QRectF&next,const QRectF&before){QQuickItem::geometryChange(next,before);updateSurface();if(next.width()!=before.width())debounce.start();}
void HtmlSurface::setReady(bool value){if(presented==value)return;presented=value;emit contentReadyChanged();}
void HtmlSurface::setBackground(const QColor&value){if(!value.isValid()||backgroundColour==value)return;backgroundColour=value;emit backgroundChanged();updateSurface();}
void HtmlSurface::setReveal(qreal value){if(!std::isfinite(value))return;value=std::clamp(value,qreal(0),qreal(1));if(revealAmount==value)return;revealAmount=value;emit revealChanged();updateSurface();}
void HtmlSurface::setDocument(const QString&value){if(page==value)return;page=value;++generation;ready=false;setReady(false);failure.clear();emit errorChanged();emit documentChanged();
#ifdef Q_OS_WIN
    if(surface){api().visible(surface,false);if(displayed){displayed=false;emit nativeVisibleChanged();}++adapterRevision;auto bytes=page.toUtf8();api().document(surface,reinterpret_cast<const unsigned char*>(bytes.constData()),bytes.size());}
#endif
    updateSurface();
}
void HtmlSurface::setZoom(qreal value){value=std::clamp(value,qreal(.5),qreal(3));if(scale==value)return;scale=value;emit zoomChanged();updateSurface();debounce.start();}
void HtmlSurface::setBlocked(bool value){if(covered==value)return;covered=value;emit blockedChanged();updateSurface();}
void HtmlSurface::setOccluders(const QVariantList&value){if(masks==value)return;masks=value;emit occludersChanged();updateSurface();}
void HtmlSurface::ancestors(){
    for(auto c:connections)disconnect(c);connections.clear();
    for(auto p=parentItem();p;p=p->parentItem()){
        connections.append(connect(p,&QQuickItem::xChanged,this,&HtmlSurface::updateSurface));connections.append(connect(p,&QQuickItem::yChanged,this,&HtmlSurface::updateSurface));
        connections.append(connect(p,&QQuickItem::widthChanged,this,&HtmlSurface::updateSurface));connections.append(connect(p,&QQuickItem::heightChanged,this,&HtmlSurface::updateSurface));connections.append(connect(p,&QQuickItem::visibleChanged,this,&HtmlSurface::updateSurface));
    }
    if(window()){connections.append(connect(window(),&QWindow::visibleChanged,this,&HtmlSurface::updateSurface));connections.append(connect(window(),&QWindow::visibilityChanged,this,&HtmlSurface::updateSurface));connections.append(connect(window(),&QWindow::screenChanged,this,&HtmlSurface::updateSurface));}
}
void HtmlSurface::release(){++nativeEpoch;debounce.stop();ready=false;setReady(false);if(creating){releasePending=true;return;}
#ifdef Q_OS_WIN
    if(surface){api().destroy(surface);surface=nullptr;--nativeSurfaceCount;}
#endif
    if(displayed){displayed=false;emit nativeVisibleChanged();}
}
void HtmlSurface::updateSurface(){
    if(!complete||!window()||creating)return;
    auto bounds=mapRectToScene(boundingRect());auto clip=bounds.intersected(QRectF(QPointF(),window()->size()));
    for(auto p=parentItem();p;p=p->parentItem())if(p->clip())clip=clip.intersected(p->mapRectToScene(p->boundingRect()));
    if(!isVisible()||!window()->isVisible()||window()->visibility()==QWindow::Minimized||clip.isEmpty())clip={};
    const auto nextTop=std::max(qreal(0),clip.y()-bounds.y()),nextSpan=std::max(qreal(0),clip.height());
    if(top!=nextTop||span!=nextSpan){top=nextTop;span=nextSpan;emit viewportChanged();}
#ifdef Q_OS_WIN
    if(!lifetimeActive||!isVisible()||!window()->isVisible()||window()->visibility()==QWindow::Minimized||clip.isEmpty()||page.isEmpty()){release();return;}
    const bool show=isVisible()&&window()->isVisible()&&!covered&&!clip.isEmpty()&&width()>1&&!page.isEmpty();
    if(!surface&&show&&failure.isEmpty()){
        QElapsedTimer creation;creation.start();
        if(!runtime)runtime=std::make_unique<ReaderRuntime>();
        if(!api().valid()||!runtime->isValid()){failure="HTML preview component is unavailable: "+api().library.errorString();emit errorChanged();return;}
        auto bytes=page.toUtf8(),directory=runtime->path().toUtf8();
        // The adapter initializes asynchronously. Keep the host's reentrancy
        // guard as well so a synchronous failure/lifetime callback cannot
        // overwrite ownership with a second native surface.
        creating=true;releasePending=false;const auto version=generation;
        // WebView2 documents a startup white-frame issue when its background
        // is configured only after controller creation. Seed the creation-time
        // colour too; subsequent palette updates use the per-controller API.
        qputenv("WEBVIEW2_DEFAULT_BACKGROUND_COLOR",QByteArray::number(backgroundColour.rgb(),16).rightJustified(8,'0'));
        surface=api().create(qintptr(window()->winId()),reinterpret_cast<const unsigned char*>(bytes.constData()),bytes.size(),reinterpret_cast<const unsigned char*>(directory.constData()),directory.size(),&HtmlSurface::callback,this);
        initialization=creation.elapsed();creating=false;emit initializationChanged();
        if(!surface){failure="Could not create the HTML view. Microsoft Edge WebView2 Runtime is required.";emit errorChanged();return;}
        ++nativeSurfaceCount;
        adapterRevision=0;api().background(surface,backgroundColour.rgba());api().reveal(surface,revealAmount);
        if(releasePending||!lifetimeActive){releasePending=false;release();return;}
        if(version!=generation){++adapterRevision;const auto current=page.toUtf8();api().document(surface,reinterpret_cast<const unsigned char*>(current.constData()),current.size());}
        QTimer::singleShot(0,this,[this]{updateSurface();if(ready)measure();});
    }
    if(!surface)return;
    const auto dpr=window()->devicePixelRatio();QList<int> rects;
    auto append=[&](QRectF r){rects<<int(std::floor(r.x()*dpr))<<int(std::floor(r.y()*dpr))<<int(std::ceil(r.width()*dpr))<<int(std::ceil(r.height()*dpr));};
    append(bounds);append(clip);for(const auto&mask:masks.mid(0,8))append(mask.toRectF());
    api().background(surface,backgroundColour.rgba());api().geometry(surface,rects.data(),rects.size()/4,scale,dpr);api().reveal(surface,revealAmount);
    const bool expose=show&&presented&&revealAmount>0;api().visible(surface,expose);
    if(displayed!=expose){displayed=expose;emit nativeVisibleChanged();}
#endif
}
void HtmlSurface::measure(){
#ifdef Q_OS_WIN
    if(surface&&ready)api().measure(surface,qRound(width()),generation);
#endif
}
void HtmlSurface::callback(void*context,const unsigned char*bytes,size_t size){
    if(size>65536)return;auto self=static_cast<HtmlSurface*>(context);QPointer<HtmlSurface>guard(self);QByteArray copy(reinterpret_cast<const char*>(bytes),size);
    const auto epoch=self->nativeEpoch;
    QMetaObject::invokeMethod(self,[guard,copy,epoch]{if(guard&&guard->nativeEpoch==epoch)guard->event(copy);},Qt::QueuedConnection);
}
void HtmlSurface::event(const QByteArray&bytes){auto e=QJsonDocument::fromJson(bytes).object();auto kind=e["kind"].toString();
    if(kind=="loaded"&&e["revision"].toInteger()==qint64(adapterRevision)){ready=true;measure();}
    else if(kind=="height"&&ready&&e["generation"].toInteger()==qint64(generation)&&e["width"].toInt()==qRound(width())){const auto h=e["height"].toDouble()*scale;if(std::isfinite(h)){if(h!=measuredHeight){measuredHeight=std::clamp(h,qreal(24),qreal(60000));emit contentHeightChanged();}const bool first=!presented;setReady(true);updateSurface();if(first)emit loaded();}}
    else if(kind=="link")emit linkActivated(e["url"].toString());
    else if(kind=="wheel")emit wheel(e["delta"].toDouble()*scale);
    else if(kind=="error"){ready=false;setReady(false);failure=e["message"].toString();emit errorChanged();updateSurface();}
}
