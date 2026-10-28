#include "attachment_preview.h"
#include "bridge.h"
#include <QCryptographicHash>
#include <QJsonDocument>
#include <QMimeDatabase>
#include <QMutexLocker>
#include <limits>
namespace {constexpr int Cards=32,Metadata=64;}
QString AttachmentPreview::watchThumbnail(const QVariantMap&source,const QString&name,const QString&mime,qint64 size){
    const auto type=mime.isEmpty()||mime=="application/octet-stream"?QMimeDatabase().mimeTypeForFile(name,QMimeDatabase::MatchExtension).name():mime;
    if(size>8*1024*1024||size<0||!type.startsWith("image/")||type.contains("svg")||source.isEmpty())return {};
    const auto key=QString::fromLatin1(QCryptographicHash::hash(QJsonDocument::fromVariant(source).toJson(QJsonDocument::Compact),QCryptographicHash::Sha256).toHex());
    if(!thumbs.contains(key)){
        pruneThumbnails();
        if(thumbs.size()>=Metadata)return {};
        thumbs.insert(key,Thumb{source});
    }
    auto&entry=thumbs[key];entry.refs++;entry.used=++thumbClock;
    if(!entry.done&&!entry.request&&!thumbQueue.contains(key)){thumbQueue.enqueue(key);pumpThumbnails();}
    return key;
}
QString AttachmentPreview::thumbnailSource(const QString&key)const{QMutexLocker lock(&store->mutex);return store->thumbnails.contains(key)?"image://attachment/thumb-"+key:QString();}
qint64 AttachmentPreview::thumbnailBytes()const{QMutexLocker lock(&store->mutex);qint64 size=0;for(const auto&image:store->thumbnails)size+=image.sizeInBytes();return size;}
void AttachmentPreview::releaseThumbnail(const QString&key){
    auto entry=thumbs.find(key);if(entry==thumbs.end()||entry->refs<1)return;
    if(--entry->refs==0){thumbQueue.removeAll(key);if(entry->request){mail->send("cancel",{{"request",entry->request}});thumbRequests.remove(entry->request);retiredRequests.insert(entry->request);while(retiredRequests.size()>64)retiredRequests.erase(retiredRequests.begin());entry->request=0;}
        // Received-mail pixels belong to the visible message, never the scrolling
        // composer LRU. Drop them as soon as its last card releases ownership.
        if(!entry->source.contains("session")){QMutexLocker lock(&store->mutex);store->thumbnails.remove(key);thumbs.erase(entry);}
        pruneThumbnails();pumpThumbnails();}
}
void AttachmentPreview::pumpThumbnails(){
    while(thumbRequests.size()<2&&!thumbQueue.isEmpty()){
        const auto key=thumbQueue.dequeue();auto entry=thumbs.find(key);if(entry==thumbs.end()||entry->refs<1||entry->done||entry->request)continue;
        auto data=entry->source;data["token"]=key;entry->request=mail->send("attachmentThumbnail",data);
        if(entry->request)thumbRequests.insert(entry->request,key);else entry->done=true;
    }
}
void AttachmentPreview::pruneThumbnails(){
    // Keep a small RAM LRU for horizontal scrolling. Active cards never lose
    // their pixels to a background card. Once the window hides, drop all
    // unreferenced pixels; no expiration timer runs at idle.
    const int limit=mail->windowVisible()?Cards:0;
    for(;;){
        QMutexLocker lock(&store->mutex);
        if(store->thumbnails.size()<=limit&&thumbs.size()<Metadata)break;
        QString oldest;quint64 used=std::numeric_limits<quint64>::max();
        for(auto i=thumbs.cbegin();i!=thumbs.cend();++i)if(i->refs==0&&i->request==0&&i->used<used){oldest=i.key();used=i->used;}
        if(oldest.isEmpty())break;
        store->thumbnails.remove(oldest);thumbs.remove(oldest);
    }
}
bool AttachmentPreview::consumeThumbnail(const QString&kind,const QJsonObject&data,quint64 id){
    if(kind=="error"&&retiredRequests.remove(id))return true;
    if(kind!="attachmentThumbnail"&&!(kind=="error"&&thumbRequests.contains(id)))return false;
    const auto key=thumbRequests.take(id);if(key.isEmpty())return true;
    auto entry=thumbs.find(key);if(entry==thumbs.end())return true;
    entry->request=0;entry->done=true;
    if(kind=="attachmentThumbnail"&&data["token"].toString()==key){
        const int w=data["width"].toInt(),h=data["height"].toInt();const auto encoded=data["rgba"].toString();
        if(w>0&&h>0&&w<=128&&h<=96&&encoded.size()<=128*96*4/3*4+8){
            const auto bytes=QByteArray::fromBase64(encoded.toLatin1(),QByteArray::AbortOnBase64DecodingErrors);
            if(bytes.size()==w*h*4){
                QImage image(reinterpret_cast<const uchar*>(bytes.constData()),w,h,QImage::Format_RGBA8888);QMutexLocker lock(&store->mutex);
                if(store->thumbnails.size()>=Cards){QString oldest;quint64 used=std::numeric_limits<quint64>::max();for(auto i=thumbs.cbegin();i!=thumbs.cend();++i)if(!i->refs&&store->thumbnails.contains(i.key())&&i->used<used){used=i->used;oldest=i.key();}if(!oldest.isEmpty()){store->thumbnails.remove(oldest);thumbs[oldest].done=false;}}
                if(store->thumbnails.size()<Cards)store->thumbnails.insert(key,image.copy());
            }
        }
    }
    pruneThumbnails();emit thumbnailChanged(key);pumpThumbnails();return true;
}
