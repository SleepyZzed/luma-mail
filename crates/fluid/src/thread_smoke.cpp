#include "bridge.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QJsonDocument>
#include <QSaveFile>
#include <QFileInfo>
class ThreadSmoke:public QObject {
    Q_OBJECT
    Bridge*mail;QQuickWindow*window;QJsonArray checks;int stage=0;bool failed=false,finished=false;
    QQuickItem*find(QQuickItem*item,const QString&name){if(item->objectName()==name)return item;for(auto child:item->childItems())if(auto result=find(child,name))return result;return nullptr;}
    int count(QQuickItem*item,const QString&prefix){int n=item->objectName().startsWith(prefix)?1:0;for(auto child:item->childItems())n+=count(child,prefix);return n;}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void finish(){if(finished)return;finished=true;QJsonObject result{{"passed",!failed},{"checks",checks},{"syntheticData",true},{"totalMessages",2503},{"headerWindow",32}};if(auto path=qEnvironmentVariable("LUMA_THREAD_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(result).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(result).toJson(QJsonDocument::Compact);mail->quit(failed?2:0);}
    void update(const QJsonObject&meta,const QJsonArray&headers){
        QJsonObject data{{"key",QJsonObject::fromVariantMap(mail->reader()["key"].toMap())},{"reading",qint64(mail->readerRequest)},{"window",meta},{"messages",headers}};
        mail->receive(QJsonObject{{"id",0},{"event","threadUpdated"},{"data",data}});
    }
    void settled(){
        auto meta=mail->reader()["window"].toMap();
        check("native thread delegates stay bounded to 32 at stage "+QString::number(stage),count(window->contentItem(),"threadMessage-")==32&&mail->messages()->rowCount()==32);
        check("collapsed messages create no body text documents at stage "+QString::number(stage),count(window->contentItem(),"readBody-")==1);
        const auto latest=find(window->contentItem(),"threadMessage-"+mail->messages()->at(31)["id"].toString());const auto top=latest?latest->mapToScene(QPointF()).y():-1;
        check("opening a long page springs its latest message into view at stage "+QString::number(stage),top>=0&&top<window->height()-80);

        if(stage==0){check("a chain beyond the old 1000-row cap opens its actual latest message",meta["total"].toInt()==2503&&meta["start"].toInt()==2472&&mail->messages()->at(31)["id"]=="preview1");stage=1;mail->pageReader("earlier");}
        else if(stage==1){check("Earlier changes only the requested bounded header window",meta["end"].toInt()==2471&&meta["later"].toBool()&&!mail->messages()->at(31)["text"].toString().isEmpty());stage=2;mail->pageReader("latest");}
        else if(stage==2){
            check("Latest restores the current end of the conversation",meta["end"].toInt()==2503&&mail->messages()->at(31)["id"]=="preview1");
            auto old=QJsonObject::fromVariantMap(meta);old["revision"]=old["revision"].toInteger()-1;
            update(old,QJsonArray{QJsonObject{{"id","stale"}}});
            check("late metadata cannot restore a previous header window",mail->messages()->rowCount()==32&&mail->messages()->at(31)["id"]=="preview1");
            QJsonArray headers;for(int i=1;i<32;++i){auto header=QJsonObject::fromVariantMap(mail->messages()->at(i));for(const auto field:{"text","document","links"})header.remove(field);headers.append(header);}headers.append(QJsonObject{{"id","incoming"},{"from","Maya <maya@example.com>"},{"unread",true}});
            auto next=QJsonObject::fromVariantMap(meta);next["total"]=2504;next["start"]=2473;next["end"]=2504;
            update(next,headers);
            stage=3;QTimer::singleShot(700,this,[this]{check("an incoming reply preserves the body being read",mail->messages()->at(30)["id"]=="preview1"&&!mail->messages()->at(30)["text"].toString().isEmpty()&&count(window->contentItem(),"readBody-")==1);check("new reply headers stay unread without automatic body fetching",mail->messages()->at(31)["unread"].toBool()&&!mail->messages()->at(31).contains("text"));mail->closeReader();QTimer::singleShot(900,this,[this]{check("closing releases every thread delegate and body",mail->messages()->rowCount()==0&&count(window->contentItem(),"readBody-")==0&&count(window->contentItem(),"threadMessage-")==0);const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");check("paging creates no mailbox or draft archive",!QFileInfo::exists(root+"/cache/bodies")&&!QFileInfo::exists(root+"/config/composer"));finish();});});
        }
    }
public:
    ThreadSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        QTimer::singleShot(22000,this,[this]{if(!finished){check("thread paging finishes before its deadline",false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(300,this,[this]{auto inbox=window->findChild<QQuickItem*>("inboxPane");QMetaObject::invokeMethod(inbox,"openIndex",Q_ARG(QVariant,1),Q_ARG(QVariant,0));});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&){if(finished)return;if(kind=="error"){check("thread commands complete without an engine error",false);finish();}else if(kind=="body"&&stage<3)QTimer::singleShot(700,this,[this]{settled();});});
    }
};
void startThreadSmoke(Bridge*m,QQuickWindow*w){new ThreadSmoke(m,w);}
#include "thread_smoke.moc"
