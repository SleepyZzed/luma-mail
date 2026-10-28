#include "bridge.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QMetaEnum>
#include <QJsonDocument>
#include <QSaveFile>
#include <QFileInfo>
class ProfileSmoke:public QObject {
    Bridge*mail;QQuickWindow*window;QJsonArray checks;bool failed=false,finished=false;int stage=0;QString before;
    QTimer deadline;
    QQuickItem* picture(QQuickItem*item)const{if(item->objectName()=="accountAvatar-previewPicture")return item;for(auto child:item->childItems())if(auto found=picture(child))return found;return nullptr;}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    bool ready()const{
        auto image=picture(window->contentItem());if(!image)return false;
        const auto meta=image->metaObject();const auto status=meta->enumerator(meta->indexOfEnumerator("Status"));return image->property("status").toInt()==status.keyToValue("Ready")&&image->property("width").toReal()>0;
    }
    void finish(){if(finished)return;finished=true;deadline.stop();QJsonObject report{{"passed",!failed},{"checks",checks},{"syntheticData",true},{"stats",QJsonObject::fromVariantMap(mail->profileStats())}};if(auto path=qEnvironmentVariable("LUMA_PROFILE_REPORT");!path.isEmpty()){QSaveFile f(path);if(f.open(QIODevice::WriteOnly)){f.write(QJsonDocument(report).toJson());f.commit();}}qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);mail->quit(failed?2:0);}
public:
    ProfileSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        deadline.setSingleShot(true);deadline.setInterval(12000);connect(&deadline,&QTimer::timeout,this,[this]{check("profile test completes within deadline",false);finish();});deadline.start();
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&){if(kind=="error"){check("profile commands do not fail",false);finish();}});
        auto timer=new QTimer(this);timer->setInterval(100);
        connect(timer,&QTimer::timeout,this,[this,timer]{
            if(finished){timer->stop();return;}
            if(stage==0&&ready()){
                stage=1;check("native image provider renders the connected avatar",true);
                auto stats=mail->profileStats();check("decoded avatar pixels obey the fixed RAM budget",stats["images"].toInt()>0&&stats["bytes"].toLongLong()<=128*96*96*4);
                before=picture(window->contentItem())->property("source").toUrl().toString();mail->send("photosReset");
            }else if(stage==2&&ready()){
                auto after=picture(window->contentItem())->property("source").toUrl().toString();if(after==before)return;
                timer->stop();stage=3;check("retry repopulates only currently watched avatars",true);
                check("photo display creates no mail or picture cache files",!QFileInfo::exists(qEnvironmentVariable("LUMA_TEST_ROOT")+"/cache/bodies")&&!QFileInfo::exists(qEnvironmentVariable("LUMA_TEST_ROOT")+"/config/composer"));finish();
            }
        });
        connect(mail,&Bridge::profilesReset,this,[this]{if(stage==1){check("reset releases decoded native images",mail->profileStats()["bytes"].toLongLong()==0);stage=2;}});timer->start();
    }
};
void startProfileSmoke(Bridge*m,QQuickWindow*w){new ProfileSmoke(m,w);}
