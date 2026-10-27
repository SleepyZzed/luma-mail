#include "html_surface.h"
#include <QQuickWindow>
#include <QLibrary>
#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPointer>
#include <QFileInfo>
#include <cmath>
#ifdef Q_OS_WIN
namespace {
struct Api {
    QLibrary loader,library;using Callback=void(*)(void*,const unsigned char*,size_t);
    using Create=void*(*)(qintptr,const unsigned char*,size_t,const unsigned char*,size_t,Callback,void*);
    Create create=nullptr;void(*destroy)(void*)=nullptr;void(*document)(void*,const unsigned char*,size_t)=nullptr;
    void(*visible)(void*,bool)=nullptr;void(*geometry)(void*,const int*,size_t,double,double)=nullptr;void(*measure)(void*,int,quint64)=nullptr;
    Api(){auto path=qEnvironmentVariable("LUMA_WEBVIEW");if(path.isEmpty())path=QCoreApplication::applicationDirPath()+"/luma_webview.dll";
        loader.setFileName(QFileInfo(path).absolutePath()+"/WebView2Loader.dll");loader.load();library.setFileName(path);if(!library.load())return;
        create=reinterpret_cast<Create>(library.resolve("luma_webview_create"));destroy=reinterpret_cast<decltype(destroy)>(library.resolve("luma_webview_destroy"));document=reinterpret_cast<decltype(document)>(library.resolve("luma_webview_document"));visible=reinterpret_cast<decltype(visible)>(library.resolve("luma_webview_visible"));geometry=reinterpret_cast<decltype(geometry)>(library.resolve("luma_webview_geometry"));measure=reinterpret_cast<decltype(measure)>(library.resolve("luma_webview_measure"));}
    bool valid()const{return create&&destroy&&document&&visible&&geometry&&measure;}
};
Api&api(){static Api instance;return instance;}
}
#endif
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
void HtmlSurface::setDocument(const QString&value){if(page==value)return;page=value;++generation;ready=false;emit documentChanged();
#ifdef Q_OS_WIN
    if(surface){auto bytes=page.toUtf8();api().document(surface,reinterpret_cast<const unsigned char*>(bytes.constData()),bytes.size());}
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
    if(window()){connections.append(connect(window(),&QWindow::visibleChanged,this,&HtmlSurface::updateSurface));connections.append(connect(window(),&QWindow::screenChanged,this,&HtmlSurface::updateSurface));}
}
void HtmlSurface::release(){debounce.stop();ready=false;
#ifdef Q_OS_WIN
    if(surface){api().destroy(surface);surface=nullptr;}
#endif
    if(displayed){displayed=false;emit nativeVisibleChanged();}
}
void HtmlSurface::updateSurface(){
    if(!complete||!window())return;
    auto bounds=mapRectToScene(boundingRect());auto clip=bounds.intersected(QRectF(QPointF(),window()->size()));
    for(auto p=parentItem();p;p=p->parentItem())if(p->clip())clip=clip.intersected(p->mapRectToScene(p->boundingRect()));
    const auto nextTop=std::max(qreal(0),clip.y()-bounds.y()),nextSpan=std::max(qreal(0),clip.height());
    if(top!=nextTop||span!=nextSpan){top=nextTop;span=nextSpan;emit viewportChanged();}
#ifdef Q_OS_WIN
    const bool show=isVisible()&&window()->isVisible()&&!covered&&!clip.isEmpty()&&width()>1&&!page.isEmpty();
    if(!surface&&show&&failure.isEmpty()){
        if(!runtime)runtime=std::make_unique<ReaderRuntime>();
        if(!api().valid()||!runtime->isValid()){failure="HTML preview component is unavailable: "+api().library.errorString();emit errorChanged();return;}
        auto bytes=page.toUtf8(),directory=runtime->path().toUtf8();
        surface=api().create(qintptr(window()->winId()),reinterpret_cast<const unsigned char*>(bytes.constData()),bytes.size(),reinterpret_cast<const unsigned char*>(directory.constData()),directory.size(),&HtmlSurface::callback,this);
        if(!surface){failure="Could not create the HTML view. Microsoft Edge WebView2 Runtime is required.";emit errorChanged();return;}
    }
    if(!surface)return;
    const auto dpr=window()->devicePixelRatio();QList<int> rects;
    auto append=[&](QRectF r){rects<<int(std::floor(r.x()*dpr))<<int(std::floor(r.y()*dpr))<<int(std::ceil(r.width()*dpr))<<int(std::ceil(r.height()*dpr));};
    append(bounds);append(clip);for(const auto&mask:masks.mid(0,8))append(mask.toRectF());
    api().geometry(surface,rects.data(),rects.size()/4,scale,dpr);api().visible(surface,show);
    if(displayed!=show){displayed=show;emit nativeVisibleChanged();}
#endif
}
void HtmlSurface::measure(){
#ifdef Q_OS_WIN
    if(surface&&ready)api().measure(surface,qRound(width()),generation);
#endif
}
void HtmlSurface::callback(void*context,const unsigned char*bytes,size_t size){
    if(size>65536)return;auto self=static_cast<HtmlSurface*>(context);QPointer<HtmlSurface>guard(self);QByteArray copy(reinterpret_cast<const char*>(bytes),size);
    QMetaObject::invokeMethod(self,[guard,copy]{if(guard)guard->event(copy);},Qt::QueuedConnection);
}
void HtmlSurface::event(const QByteArray&bytes){auto e=QJsonDocument::fromJson(bytes).object();auto kind=e["kind"].toString();
    if(kind=="loaded"){ready=true;measure();emit loaded();}
    else if(kind=="height"&&e["generation"].toInteger()==qint64(generation)&&e["width"].toInt()==qRound(width())){const auto h=e["height"].toDouble()*scale;if(std::isfinite(h)&&h!=measuredHeight){measuredHeight=std::clamp(h,qreal(24),qreal(60000));emit contentHeightChanged();}}
    else if(kind=="link")emit linkActivated(e["url"].toString());
    else if(kind=="wheel")emit wheel(e["delta"].toDouble()*scale);
    else if(kind=="error"){failure=e["message"].toString();emit errorChanged();}
}
