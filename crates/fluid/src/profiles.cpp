#include "profiles.h"
#include "bridge.h"
#include <QUuid>
#include <limits>
#include <algorithm>
QString Bridge::watchProfile(const QString&account,const QString&sender){
    if(!profileImages||account.isEmpty()||account.size()>128||sender.isEmpty()||sender.size()>2048)return {};
    const auto key=account+QChar(0)+sender;
    if(!profiles.contains(key)&&profiles.size()>=256){
        QString oldest;qint64 age=std::numeric_limits<qint64>::max();for(auto i=profiles.begin();i!=profiles.end();++i)if(!i->refs&&i->touched<age){oldest=i.key();age=i->touched;}
        if(oldest.isEmpty())return {};releaseProfileEntry(oldest);
    }
    auto &entry=profiles[key];entry.account=account;entry.sender=sender;++entry.refs;entry.touched=++profileSerial;
    if(!entry.pending&&(entry.expires<=profileClock.elapsed()||(!entry.source.isEmpty()&&!profileImages->contains(entry.source)))){
        entry.token=QUuid::createUuid().toString(QUuid::WithoutBraces);entry.pending=true;profileTokens[entry.token]=key;
        entry.request=send("photoRequest",{{"account",account},{"sender",sender},{"token",entry.token}});
        if(entry.request)profileRequests[entry.request]=key;
        else{profileTokens.remove(entry.token);entry.pending=false;entry.expires=profileClock.elapsed()+1000;}
    }
    return key;
}
void Bridge::releaseProfile(const QString&key){
    auto i=profiles.find(key);if(i==profiles.end())return;i->refs=std::max(0,i->refs-1);i->touched=++profileSerial;
    if(!i->refs&&i->pending)releaseProfileEntry(key);
}
QString Bridge::profileSource(const QString&key)const{auto i=profiles.constFind(key);return i!=profiles.cend()&&profileImages&&profileImages->contains(i->source)?"image://profiles/"+i->source:QString();}
void Bridge::releaseProfileEntry(const QString&key){
    auto i=profiles.find(key);if(i==profiles.end())return;const auto source=i->source;
    if(i->pending){send("photoCancel",{{"token",i->token}});profileTokens.remove(i->token);profileRequests.remove(i->request);}
    profiles.erase(i);bool shared=false;for(const auto&e:profiles)shared|=e.source==source;
    if(!shared&&profileImages)profileImages->remove(source);
}
void Bridge::resetProfiles(){
    profiles.clear();profileTokens.clear();profileRequests.clear();if(profileImages)profileImages->clear();emit profilesReset();
}
QVariantMap Bridge::profileStats()const{int pending=0,missing=0;for(const auto&e:profiles){pending+=e.pending;missing+=!e.pending&&e.source.isEmpty();}return {{"images",profileImages?profileImages->count():0},{"bytes",profileImages?profileImages->bytes():0},{"pending",pending},{"missing",missing}};}
bool Bridge::profileEvent(const QString&kind,const QJsonObject&data,quint64 id){
    if(kind=="photosReset"){resetProfiles();return true;}
    if(kind=="error"&&profileRequests.contains(id)){
        const auto key=profileRequests.take(id);auto i=profiles.find(key);if(i!=profiles.end()){profileTokens.remove(i->token);i->pending=false;i->expires=profileClock.elapsed()+60000;i->note=data["message"].toString();emit profileChanged(key);}return true;
    }
    if(kind!="photo")return false;
    QStringList keys;for(auto token:data["requests"].toArray()){const auto key=profileTokens.take(token.toString());if(!key.isEmpty()&&profiles.contains(key))keys.append(key);}
    if(keys.isEmpty())return true;
    const auto source=data["source"].toString();bool available=false;
    if(profileImages&&!source.isEmpty()&&data["rgba"].toString().size()<=50000){
        while(profileImages->count()>=128){
            QString candidate;qint64 age=std::numeric_limits<qint64>::max();
            for(auto i=profiles.cbegin();i!=profiles.cend();++i){if(i->source.isEmpty()||i->refs)continue;bool active=false;for(const auto&e:profiles)if(e.source==i->source&&e.refs)active=true;if(!active&&i->touched<age){candidate=i->source;age=i->touched;}}
            if(candidate.isEmpty())break;QStringList victims;for(auto i=profiles.cbegin();i!=profiles.cend();++i)if(i->source==candidate)victims.append(i.key());for(const auto&key:victims)releaseProfileEntry(key);
        }
        const auto bytes=QByteArray::fromBase64(data["rgba"].toString().toLatin1(),QByteArray::AbortOnBase64DecodingErrors);
        available=profileImages->insert(source,bytes,data["width"].toInt(),data["height"].toInt());
    }
    QStringList oldSources;
    for(const auto&key:keys){auto i=profiles.find(key);if(i==profiles.end())continue;oldSources.append(i->source);profileRequests.remove(i->request);i->pending=false;i->source=available?source:QString();i->note=data["note"].toString();i->expires=profileClock.elapsed()+std::clamp(data["ttlSeconds"].toInteger(60),qint64(10),qint64(600))*1000;i->touched=++profileSerial;emit profileChanged(key);}
    for(const auto&old:oldSources){if(old.isEmpty())continue;bool owned=false;for(const auto&e:profiles)owned|=e.source==old;if(!owned&&profileImages)profileImages->remove(old);}
    return true;
}
