#include "bridge.h"
#include <QApplication>
#include <QClipboard>
#include <QMimeData>
#include <QImage>
#include <QIODevice>
#include <QPointer>
#include <QThreadPool>
#include <QUuid>
namespace {
constexpr qint64 Limit=16*1024*1024,Chunk=256*1024;
class BoundedOutput:public QIODevice {
    std::shared_ptr<std::atomic_bool> cancelled;
public:
    QByteArray bytes;
    explicit BoundedOutput(std::shared_ptr<std::atomic_bool> flag):cancelled(std::move(flag)){open(QIODevice::WriteOnly);}
    qint64 readData(char*,qint64)override{return -1;}
    qint64 writeData(const char*data,qint64 count)override{if(cancelled->load()||count<0||count>Limit-bytes.size())return -1;bytes.append(data,count);return count;}
};
}
bool Bridge::interceptImagePaste(){
    const auto mime=m_preview&&clipboardFixture?clipboardFixture.data():qApp->clipboard()->mimeData();if(!mime)return false;
    const QStringList types{"image/png","image/jpeg","image/webp","image/gif"};
    bool image=mime->hasImage();for(const auto&type:types)image|=mime->hasFormat(type);if(!image)return false;
    if(clipboardBusy())return true;
    if(clipboardEncoding){m_error="The previous clipboard image is finishing cancellation. Try paste again shortly.";emit errorChanged();return true;}
    if(m_composer.value("session").toString().isEmpty()||m_composer.value("locked").toBool()||m_composer.value("inputBusy").toBool()||!QStringList{"editing","uploading"}.contains(m_composer.value("state").toString())){
        m_error="Wait for the current operation before pasting an image.";emit errorChanged();return true;
    }
    clipboardSession=m_composer.value("session").toString();clipboardToken=QUuid::createUuid().toString(QUuid::WithoutBraces);clipboardOffset=0;clipboardReady=false;clipboardAcked=false;clipboardCancelled=false;clipboardBegan=false;
    clipboardRequest=send("composeClipboardBegin",{{"session",clipboardSession},{"token",clipboardToken}});emit clipboardChanged();
    for(const auto&type:types)if(mime->hasFormat(type)){
        auto bytes=mime->data(type);
        if(bytes.size()>Limit){m_error="Pasted images must fit within 16 MiB. Attach a saved file to use Drive for larger images.";emit errorChanged();cancelClipboard();return true;}
        if(!bytes.isEmpty()){clipboardBytes=std::move(bytes);clipboardReady=true;pumpClipboard();return true;}
    }
    const auto bitmap=m_preview&&clipboardFixture?qvariant_cast<QImage>(mime->imageData()):qApp->clipboard()->image();
    if(bitmap.isNull()||bitmap.width()>16384||bitmap.height()>16384||qint64(bitmap.width())*bitmap.height()>32000000){m_error="This clipboard image exceeds the image safety limit. Attach it as a file instead.";emit errorChanged();cancelClipboard();return true;}
    const auto token=clipboardToken;QPointer<Bridge> guard(this);
    clipboardEncoding=true;clipboardEncodingCancel=std::make_shared<std::atomic_bool>(false);auto cancelled=clipboardEncodingCancel;
    QThreadPool::globalInstance()->start([guard,token,bitmap,cancelled]{
        BoundedOutput output(cancelled);const bool ok=bitmap.save(&output,"PNG");auto bytes=std::move(output.bytes);
        if(guard)QMetaObject::invokeMethod(guard,[guard,token,ok,bytes=std::move(bytes)]()mutable{
            if(!guard)return;guard->clipboardEncoding=false;guard->clipboardEncodingCancel.reset();
            if(guard->clipboardToken!=token||guard->clipboardCancelled)return;
            if(!ok||bytes.isEmpty()){guard->m_error="The clipboard image could not fit within 16 MiB. Attach a saved file to use Drive.";emit guard->errorChanged();guard->cancelClipboard();return;}
            guard->clipboardBytes=std::move(bytes);guard->clipboardReady=true;guard->pumpClipboard();
        },Qt::QueuedConnection);
    });
    return true;
}
void Bridge::pumpClipboard(){
    if(!clipboardBusy()||!clipboardReady||!clipboardAcked||clipboardCancelled)return;
    clipboardAcked=false;
    if(clipboardOffset<0||clipboardOffset>clipboardBytes.size()){cancelClipboard();return;}
    if(clipboardOffset==clipboardBytes.size()){
        clipboardRequest=send("composeClipboardFinish",{{"session",clipboardSession},{"token",clipboardToken}});clipboardBytes.clear();clipboardBytes.squeeze();clipboardReady=false;
    }else{
        auto chunk=clipboardBytes.mid(clipboardOffset,Chunk).toBase64();
        clipboardRequest=send("composeClipboardChunk",{{"session",clipboardSession},{"token",clipboardToken},{"offset",clipboardOffset},{"size",clipboardBytes.size()},{"bytes",QString::fromLatin1(chunk)}});
    }
}
void Bridge::cancelClipboard(){
    if(!clipboardBusy())return;clipboardCancelled=true;clipboardBytes.clear();clipboardBytes.squeeze();
    if(clipboardEncodingCancel)clipboardEncodingCancel->store(true);
    clipboardRequest=send("composeClipboardCancel",{{"session",clipboardSession},{"token",clipboardToken}});
}
void Bridge::resetClipboard(const QString&error){
    clipboardBytes.clear();clipboardBytes.squeeze();clipboardToken.clear();clipboardSession.clear();clipboardRequest=0;clipboardReady=false;clipboardAcked=false;clipboardBegan=false;clipboardCancelled=false;emit clipboardChanged();
    if(!error.isEmpty()){m_error=error;emit errorChanged();}
}
void Bridge::clipboardEvent(const QString&kind,const QJsonObject&data,quint64 id){
    if(!clipboardBusy())return;
    if(kind=="error"&&id==clipboardRequest){if(clipboardBegan)send("composeClipboardCancel",{{"session",clipboardSession},{"token",clipboardToken}});resetClipboard();return;}
    if(data["session"].toString()!=clipboardSession||data["token"].toString()!=clipboardToken)return;
    if(kind=="clipboardAck"){clipboardBegan=true;clipboardOffset=data["next"].toInteger(-1);clipboardAcked=true;pumpClipboard();}
    else if(kind=="clipboardComplete"){const bool ok=data["ok"].toBool();auto message=!ok&&!clipboardCancelled?data["message"].toString():QString();resetClipboard(message);}
}
