#include "attachment_preview.h"
#include "bridge.h"
#include <QMediaPlayer>
#include <QMediaMetaData>
#include <QAudioOutput>
#include <QVideoSink>
#include <QVideoFrame>
#include <QMutexLocker>
#include <QUuid>
#include <QJsonObject>
#include <QUrl>
#include <algorithm>
#include <QFileDialog>
namespace {constexpr qint64 Limit=24*1024*1024,Chunk=256*1024;}
QImage PreviewImageProvider::requestImage(const QString&id,QSize*size,const QSize&){QMutexLocker lock(&pixels->mutex);auto image=id.startsWith("thumb-")?pixels->thumbnails.value(id.mid(6)):(id==pixels->key?pixels->image:QImage());if(size)*size=image.size();return image;}
AttachmentPreview::AttachmentPreview(Bridge*value):QObject(value),mail(value){connect(mail,&Bridge::windowVisibleChanged,this,[this]{if(!mail->windowVisible())pruneThumbnails();});}
AttachmentPreview::~AttachmentPreview(){clearMedia();}
qint64 AttachmentPreview::duration()const{return player?player->duration():0;}
qint64 AttachmentPreview::position()const{return player?player->position():0;}
bool AttachmentPreview::playing()const{return player&&player->playbackState()==QMediaPlayer::PlayingState;}
void AttachmentPreview::clearMedia(){if(player){player->stop();player->setSourceDevice(nullptr);player.reset();}if(sink)sink->setVideoFrame(QVideoFrame());audio.reset();buffer.close();buffer.setData(QByteArray());bytes.clear();}
void AttachmentPreview::open(const QVariantMap&value,const QString&name,const QRectF&origin){close();source=value;startRect=origin;token=QUuid::createUuid().toString(QUuid::WithoutBraces);title=name;busy=true;auto data=value;data["token"]=token;request=mail->send("attachmentPreview",data);if(!request)fail("The mail engine is not connected.");emit changed();}
void AttachmentPreview::save(const QVariantMap&value,const QString&){saveMany({value});}
void AttachmentPreview::saveMany(const QVariantList&sources){
    if(saving||saveDialog||sources.isEmpty()||sources.size()>100)return;
    auto dialog=new QFileDialog;saveDialog=dialog;dialog->setAttribute(Qt::WA_DeleteOnClose);dialog->setWindowTitle(sources.size()>1?"Save all attachments to folder":"Save attachment to folder");dialog->setFileMode(QFileDialog::Directory);dialog->setOption(QFileDialog::ShowDirsOnly);dialog->setWindowModality(Qt::ApplicationModal);
    connect(dialog,&QFileDialog::fileSelected,this,[this,sources](const QString&directory){saveToken=QUuid::createUuid().toString(QUuid::WithoutBraces);saving=true;stopping=false;saveProgress=0;saveMessage="Preparing attachment download…";saveRequest=mail->send("attachmentSaveAll",{{"token",saveToken},{"directory",directory},{"attachments",sources}});if(!saveRequest){saving=false;saveMessage="The mail engine is not connected.";}emit downloadChanged();});dialog->open();
}
void AttachmentPreview::cancelDownload(){if(!saving||stopping)return;stopping=true;saveMessage="Cancelling safely…";mail->send("attachmentSaveCancel",{{"token",saveToken}});emit downloadChanged();}
void AttachmentPreview::dismissDownload(){if(!saving){saveMessage.clear();emit downloadChanged();}}
bool AttachmentPreview::consumeDownload(const QString&kind,const QJsonObject&data,quint64 id){
    if(kind=="error"&&saveRequest&&id==saveRequest){saving=false;saveRequest=0;saveMessage=data["message"].toString();emit downloadChanged();return true;}
    if((kind!="attachmentSaveProgress"&&kind!="attachmentSaveFinished")||data["token"].toString()!=saveToken)return false;
    if(kind=="attachmentSaveProgress"){const int total=data["total"].toInt();saveProgress=total>0?qreal(data["completed"].toInt())/total:0;if(!stopping)saveMessage=data["name"].toString("Saving attachments…");}
    else{saving=false;stopping=false;saveRequest=0;saveProgress=1;saveMessage=(data["cancelled"].toBool()?"Cancelled · ":"")+QString::number(data["saved"].toInt())+" attachment(s) saved";const auto errors=data["errors"].toArray();if(!errors.isEmpty())saveMessage+=" · "+errors.first().toString();}
    emit downloadChanged();return true;
}
void AttachmentPreview::close(){if(request)retiredRequests.insert(request);if(readRequest)retiredRequests.insert(readRequest);while(retiredRequests.size()>64)retiredRequests.erase(retiredRequests.begin());if(!token.isEmpty())mail->send("attachmentRelease",{{"token",token}});token.clear();title.clear();type.clear();failure.clear();content.clear();imageKey.clear();busy=false;total=0;request=readRequest=0;clearMedia();{QMutexLocker lock(&store->mutex);store->image={};store->key.clear();}emit changed();emit mediaChanged();}
void AttachmentPreview::fail(const QString&message){failure=message;busy=false;readRequest=0;mail->send("attachmentRelease",{{"token",token}});clearMedia();emit changed();emit mediaChanged();}
bool AttachmentPreview::consume(const QString&kind,const QJsonObject&data,quint64 id){
    if(consumeDownload(kind,data,id))return true;
    if(consumeThumbnail(kind,data,id))return true;
    if((kind=="attachmentPreview"||kind=="attachmentLoading"||kind=="attachmentChunk")&&(token.isEmpty()||data["token"].toString()!=token))return true;
    if(kind=="error"&&(id==request||id==readRequest)&&id){fail(data["message"].toString());return true;}
    if(!kind.startsWith("attachment")||data["token"].toString()!=token||token.isEmpty())return false;
    if(kind=="attachmentLoading"){title=data["name"].toString();type=data["kind"].toString();emit changed();return true;}
    if(kind=="attachmentPreview"){
        title=data["name"].toString();type=data["kind"].toString();
        if(type=="Image"){
            const int w=data["width"].toInt(),h=data["height"].toInt();if(w<1||h<1||w>1280||h>960||data["rgba"].toString().size()>7*1024*1024){fail("The image exceeds the preview limits.");return true;}
            auto rgba=QByteArray::fromBase64(data["rgba"].toString().toLatin1(),QByteArray::AbortOnBase64DecodingErrors);if(rgba.size()!=w*h*4){fail("Invalid image preview data.");return true;}
            QImage image(reinterpret_cast<const uchar*>(rgba.constData()),w,h,QImage::Format_RGBA8888);{QMutexLocker lock(&store->mutex);store->image=image.copy();store->key=token;}imageKey=token;busy=false;
        }else if(type=="Audio"||type=="Video"){
            total=data["size"].toInteger();if(total<1||total>Limit){fail("Media preview must fit within 24 MiB of RAM. Save it to open externally.");return true;}bytes.reserve(total);requestChunk();
        }else{content=data["text"].toString();busy=false;}
        emit changed();return true;
    }
    if(kind=="attachmentChunk"){
        const auto encoded=data["data"].toString();if(encoded.size()>Chunk/3*4+8){fail("Invalid preview chunk.");return true;}
        auto chunk=QByteArray::fromBase64(encoded.toLatin1(),QByteArray::AbortOnBase64DecodingErrors);
        if(chunk.isEmpty()||chunk.size()>Chunk||data["total"].toInteger()!=total||data["done"].toInteger()!=bytes.size()+chunk.size()||chunk.size()>total-bytes.size()){fail("The media preview transfer was interrupted.");return true;}
        bytes.append(chunk);if(bytes.size()==total){readRequest=0;mail->send("attachmentRelease",{{"token",token}});prepareMedia();busy=false;}else requestChunk();emit changed();return true;
    }
    return false;
}
void AttachmentPreview::requestChunk(){readRequest=mail->send("attachmentRead",{{"token",token},{"offset",bytes.size()}});if(!readRequest)fail("The preview transfer disconnected.");}
void AttachmentPreview::prepareMedia(){
    buffer.setData(bytes);buffer.open(QIODevice::ReadOnly);audio=std::make_unique<QAudioOutput>();audio->setVolume(loudness);player=std::make_unique<QMediaPlayer>();player->setAudioOutput(audio.get());if(sink)player->setVideoSink(sink);
    connect(player.get(),&QMediaPlayer::positionChanged,this,&AttachmentPreview::mediaChanged);connect(player.get(),&QMediaPlayer::durationChanged,this,&AttachmentPreview::mediaChanged);connect(player.get(),&QMediaPlayer::playbackStateChanged,this,&AttachmentPreview::mediaChanged);
    connect(player.get(),&QMediaPlayer::errorOccurred,this,[this](QMediaPlayer::Error,const QString&message){failure=message;busy=false;emit changed();});
    connect(player.get(),&QMediaPlayer::metaDataChanged,this,[this]{if(!player)return;const auto size=player->metaData().value(QMediaMetaData::Resolution).toSize();if(qint64(size.width())*size.height()>3840*2160){player->stop();failure="This video's dimensions exceed the in-app playback limit. Save it to open externally.";emit changed();}});
    player->setSourceDevice(&buffer,QUrl("memory:/"+QString::fromLatin1(QUrl::toPercentEncoding(title))));emit mediaChanged();
}
void AttachmentPreview::playPause(){if(!player)return;if(playing())player->pause();else player->play();}
void AttachmentPreview::pause(){if(player)player->pause();}
void AttachmentPreview::setVolume(qreal value){if(!qIsFinite(value))return;loudness=std::clamp(value,qreal(0),qreal(1));if(audio)audio->setVolume(loudness);emit mediaChanged();}
bool AttachmentPreview::hasVideoFrame()const{return sink&&sink->videoFrame().isValid();}
void AttachmentPreview::seek(qint64 value){if(player)player->setPosition(std::clamp(value,qint64(0),duration()));}
void AttachmentPreview::setVideoSink(QObject*value){sink=qobject_cast<QVideoSink*>(value);if(player)player->setVideoSink(sink);}
void AttachmentPreview::clearVideoSink(QObject*value){if(value==sink)setVideoSink(nullptr);}
