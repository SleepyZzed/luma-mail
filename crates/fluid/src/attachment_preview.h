#pragma once
#include <QObject>
#include <QVariantMap>
#include <QBuffer>
#include <QQuickImageProvider>
#include <QMutex>
#include <QImage>
#include <QJsonObject>
#include <QPointer>
#include <QRectF>
#include <memory>
#include <QHash>
#include <QSet>
#include <QQueue>
class Bridge;class QMediaPlayer;class QAudioOutput;class QVideoSink;
struct PreviewPixels {QMutex mutex;QImage image;QString key;QHash<QString,QImage>thumbnails;};
class PreviewImageProvider:public QQuickImageProvider {
    std::shared_ptr<PreviewPixels>pixels;
public:
    explicit PreviewImageProvider(std::shared_ptr<PreviewPixels>value):QQuickImageProvider(QQuickImageProvider::Image),pixels(std::move(value)){}
    QImage requestImage(const QString&id,QSize*size,const QSize&)override;
};
class AttachmentPreview:public QObject {
    Q_OBJECT
    Q_PROPERTY(bool opened READ opened NOTIFY changed)
    Q_PROPERTY(QString name READ name NOTIFY changed)
    Q_PROPERTY(QString kind READ kind NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(QString text READ text NOTIFY changed)
    Q_PROPERTY(QString image READ image NOTIFY changed)
    Q_PROPERTY(bool loading READ loading NOTIFY changed)
    Q_PROPERTY(qreal progress READ progress NOTIFY changed)
    Q_PROPERTY(qint64 duration READ duration NOTIFY mediaChanged)
    Q_PROPERTY(qint64 position READ position NOTIFY mediaChanged)
    Q_PROPERTY(bool playing READ playing NOTIFY mediaChanged)
    Q_PROPERTY(qreal volume READ volume WRITE setVolume NOTIFY mediaChanged)
    Q_PROPERTY(QRectF origin READ origin NOTIFY changed)
    Q_PROPERTY(bool downloading READ downloading NOTIFY downloadChanged)
    Q_PROPERTY(bool cancelling READ cancelling NOTIFY downloadChanged)
    Q_PROPERTY(QString downloadMessage READ downloadMessage NOTIFY downloadChanged)
    Q_PROPERTY(qreal downloadProgress READ downloadProgress NOTIFY downloadChanged)
public:
    explicit AttachmentPreview(Bridge*mail);~AttachmentPreview()override;
    bool opened()const{return !token.isEmpty();}QString name()const{return title;}QString kind()const{return type;}QString error()const{return failure;}QString text()const{return content;}
    QString image()const{return imageKey.isEmpty()?QString():"image://attachment/"+imageKey;}
    bool loading()const{return busy;}qreal progress()const{return total>0?qreal(bytes.size())/total:0;}
    QRectF origin()const{return startRect;}
    qint64 duration()const;qint64 position()const;bool playing()const;
    qreal volume()const{return loudness;}void setVolume(qreal value);
    Q_INVOKABLE void open(const QVariantMap&source,const QString&name,const QRectF&origin=QRectF());
    Q_INVOKABLE void save(const QVariantMap&source,const QString&name);
    Q_INVOKABLE void saveMany(const QVariantList&sources);
    Q_INVOKABLE void cancelDownload();Q_INVOKABLE void dismissDownload();
    bool downloading()const{return saving;}bool cancelling()const{return stopping;}
    QString downloadMessage()const{return saveMessage;}qreal downloadProgress()const{return saveProgress;}
    Q_INVOKABLE void saveCurrent(){save(source,title);}
    Q_INVOKABLE void pause();
    bool hasVideoFrame()const;
    Q_INVOKABLE void close();Q_INVOKABLE void playPause();Q_INVOKABLE void seek(qint64 position);
    Q_INVOKABLE void setVideoSink(QObject*sink);Q_INVOKABLE void clearVideoSink(QObject*sink);
    Q_INVOKABLE QString watchThumbnail(const QVariantMap&source,const QString&name,const QString&mime,qint64 size);
    Q_INVOKABLE void releaseThumbnail(const QString&key);
    Q_INVOKABLE QString thumbnailSource(const QString&key)const;
    qint64 thumbnailBytes()const;
    bool consume(const QString&kind,const QJsonObject&data,quint64 request);
    std::shared_ptr<PreviewPixels>pixels()const{return store;}
signals:void changed();void mediaChanged();void thumbnailChanged(QString key);void downloadChanged();
private:
    bool saving=false,stopping=false;QString saveToken,saveMessage;qreal saveProgress=0;quint64 saveRequest=0;QPointer<QObject>saveDialog;
    bool consumeDownload(const QString&kind,const QJsonObject&data,quint64 id);
    struct Thumb {QVariantMap source;int refs=0;quint64 request=0;bool done=false;quint64 used=0;};
    QHash<QString,Thumb>thumbs;QQueue<QString>thumbQueue;QHash<quint64,QString>thumbRequests;QSet<quint64>retiredRequests;quint64 thumbClock=0;
    void pumpThumbnails();bool consumeThumbnail(const QString&kind,const QJsonObject&data,quint64 id);void pruneThumbnails();
    void requestChunk();void fail(const QString&message);void clearMedia();void prepareMedia();
    Bridge*mail;QString token,title,type,failure,content,imageKey;QVariantMap source;
    QRectF startRect;
    bool busy=false;qint64 total=0;QByteArray bytes;quint64 request=0,readRequest=0;
    qreal loudness=.65;
    std::shared_ptr<PreviewPixels>store=std::make_shared<PreviewPixels>();
    QBuffer buffer;std::unique_ptr<QAudioOutput>audio;std::unique_ptr<QMediaPlayer>player;QPointer<QVideoSink>sink;
};
