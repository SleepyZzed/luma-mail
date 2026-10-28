#pragma once
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>
#include "runtime_cleanup.h"
#include <QTimer>
#include <QColor>
#include <memory>
class HtmlSurface:public QQuickItem {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(qint64 initializationMs READ initializationMs NOTIFY initializationChanged)
    Q_PROPERTY(QString document READ document WRITE setDocument NOTIFY documentChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(qreal contentHeight READ contentHeight NOTIFY contentHeightChanged)
    Q_PROPERTY(qreal zoom READ zoom WRITE setZoom NOTIFY zoomChanged)
    Q_PROPERTY(bool blocked READ blocked WRITE setBlocked NOTIFY blockedChanged)
    Q_PROPERTY(bool active READ active WRITE setActive NOTIFY activeChanged)
    Q_PROPERTY(bool nativeVisible READ nativeVisible NOTIFY nativeVisibleChanged)
    Q_PROPERTY(bool contentReady READ contentReady NOTIFY contentReadyChanged)
    Q_PROPERTY(QColor background READ background WRITE setBackground NOTIFY backgroundChanged)
    Q_PROPERTY(qreal reveal READ reveal WRITE setReveal NOTIFY revealChanged)
    Q_PROPERTY(qreal viewportTop READ viewportTop NOTIFY viewportChanged)
    Q_PROPERTY(qreal viewportHeight READ viewportHeight NOTIFY viewportChanged)
    Q_PROPERTY(QVariantList occluders READ occluders WRITE setOccluders NOTIFY occludersChanged)
public:
    explicit HtmlSurface(QQuickItem*parent=nullptr);~HtmlSurface()override;
    qint64 initializationMs()const{return initialization;}
    QString document()const{return page;}void setDocument(const QString&value);
    QString error()const{return failure;}qreal contentHeight()const{return measuredHeight;}
    qreal zoom()const{return scale;}void setZoom(qreal value);
    bool blocked()const{return covered;}void setBlocked(bool value);
    bool active()const{return lifetimeActive;}void setActive(bool value);
    static int liveNativeSurfaces();
    bool nativeVisible()const{return displayed;}
    bool contentReady()const{return presented;}QColor background()const{return backgroundColour;}void setBackground(const QColor&value);
    qreal reveal()const{return revealAmount;}void setReveal(qreal value);
    qreal viewportTop()const{return top;}qreal viewportHeight()const{return span;}
    QVariantList occluders()const{return masks;}void setOccluders(const QVariantList&value);
signals:
    void initializationChanged();void documentChanged();void errorChanged();void contentHeightChanged();void zoomChanged();void blockedChanged();void nativeVisibleChanged();void occludersChanged();
    void linkActivated(QString url);void wheel(qreal delta);void loaded();
    void viewportChanged();
    void activeChanged();
    void contentReadyChanged();void backgroundChanged();void revealChanged();
protected:
    void componentComplete()override;void geometryChange(const QRectF&next,const QRectF&before)override;
private:
    void ancestors();void updateSurface();void release();void measure();void event(const QByteArray&bytes);void setReady(bool value);
    static void callback(void*context,const unsigned char*bytes,size_t size);
    QString page,failure;std::unique_ptr<ReaderRuntime> runtime;QTimer debounce;QVariantList masks;
    QVector<QMetaObject::Connection> connections;void*surface=nullptr;
    qint64 initialization=0;quint64 adapterRevision=0;bool presented=false;QColor backgroundColour=QColor("#19171e");qreal revealAmount=1;
    qreal measuredHeight=48,scale=1,top=0,span=0;bool complete=false,covered=false,displayed=false,ready=false,lifetimeActive=true,creating=false,releasePending=false;quint64 generation=0,nativeEpoch=0;
};
