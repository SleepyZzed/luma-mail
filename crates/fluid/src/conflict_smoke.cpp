#include "bridge.h"
#include "rich_document.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QSaveFile>
#include <QJsonDocument>
#include <QFileInfo>
class ConflictSmoke:public QObject {
    Bridge*mail;QQuickWindow*window;QQuickItem*sheet=nullptr,*card=nullptr;QJsonArray checks;bool failed=false,finished=false;int stage=0;QString original;
    QQuickItem*find(QQuickItem*at,const QString&name){if(at->objectName()==name)return at;for(auto child:at->childItems())if(auto item=find(child,name))return item;return nullptr;}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void select(const QString&name,int value){auto item=find(window->contentItem(),name);check("version selector exists: "+name,item);if(!item)return;item->setProperty("currentIndex",value);QMetaObject::invokeMethod(item,"activated",Q_ARG(int,value));}
    void finish(){if(finished)return;finished=true;QJsonObject report{{"passed",!failed},{"checks",checks},{"syntheticData",true},{"note","Synthetic conflicting draft through the real private engine; no Gmail save/delete/send."}};if(failed&&sheet)report["diagnostics"]=QJsonObject{{"choices",QJsonValue::fromVariant(sheet->property("choices"))},{"attachments",sheet->property("attachments").toString()},{"pending",sheet->property("pending").toBool()}};if(auto path=qEnvironmentVariable("LUMA_CONFLICT_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(report).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);if(failed){const auto state=mail->composer();mail->send("composeClose",{{"session",state["session"]}});QTimer::singleShot(100,this,[this]{const auto state=mail->composer();mail->send("composeCloseAnswer",{{"session",state["session"]},{"revision",state["revision"]},{"answer","close"}});QTimer::singleShot(100,this,[this]{mail->quit(2);});});}else mail->quit();}
public:
    ConflictSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        QTimer::singleShot(15000,this,[this]{if(!finished){check("conflict review finishes within deadline",false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{mail->send("settingsUpdate",{{"draft_mode","manual"}});mail->requestCompose();});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&data){
            if(finished)return;
            if(kind=="error"){check("conflict commands succeed: "+data.value("message").toString(),false);finish();return;}
            if(kind=="composeOpened"&&stage==0){stage=1;original=data["session"].toString();check("fixture reaches real conflict guard",data["state"]=="conflict"&&data["draftId"]=="synthetic-draft");QTimer::singleShot(750,this,[this]{sheet=find(window->contentItem(),"draftCompareSheet");card=find(window->contentItem(),"composer");check("comparison surface is available",sheet&&card);if(!sheet||!card){finish();return;}QVariant form;QMetaObject::invokeMethod(card,"form",Q_RETURN_ARG(QVariant,form));QMetaObject::invokeMethod(sheet,"begin",Q_ARG(QVariant,original),Q_ARG(QVariant,form));});}
            else if(kind=="composeComparison"&&stage==1){stage=2;auto fields=data["fields"].toList();check("different subject merges while body requires explicit choice",fields.size()==6&&fields[4].toMap()["resolution"]=="gmail"&&fields[5].toMap()["resolution"]=="choose");check("comparison shows both read-only body versions",fields[5].toMap()["mine"]=="My edited message"&&fields[5].toMap()["gmail"]=="Gmail edited message");check("unresolved comparison cannot create a copy",!sheet->property("ready").toBool());QTimer::singleShot(700,this,[this]{select("mergeChoice5",1);select("mergeAttachments",1);check("explicit choices enable unsaved merged copy",sheet->property("ready").toBool());if(auto path=qEnvironmentVariable("LUMA_CONFLICT_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);QMetaObject::invokeMethod(sheet,"merge");});}
            else if(kind=="composeOpened"&&stage==2){stage=3;check("merge creates a fresh unsaved session",data["session"]!=original&&data["draftId"].isNull()&&data["state"]=="editing");check("merged copy is dirty and preserves Gmail thread routing",data["dirty"].toBool()&&data["accountLocked"].toBool());QTimer::singleShot(800,this,[this]{auto subject=find(window->contentItem(),"composeSubject");auto rich=window->findChild<RichDocument*>("composeDocument");check("chosen versions reach the actual editor",subject&&subject->property("text").toString()=="Gmail subject"&&rich&&rich->nativeDocument()->toPlainText()=="My edited message");check("comparison body data releases after the slide closes",!sheet->isVisible()&&sheet->property("review").toMap().isEmpty());QMetaObject::invokeMethod(card,"requestClose");});}
            else if(kind=="composeClosed"&&stage==3){stage=4;const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");check("conflict resolution creates no disk draft or mail archive",!QFileInfo::exists(root+"/config/composer")&&!QFileInfo::exists(root+"/cache/bodies"));finish();}
        });
    }
};
void startConflictSmoke(Bridge*m,QQuickWindow*w){new ConflictSmoke(m,w);}
