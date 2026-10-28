#include "bridge.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QSaveFile>
#include <QJsonDocument>
#include <QFileInfo>
class SearchSmoke:public QObject {
    Bridge*mail;QQuickWindow*window;QQuickItem*sheet=nullptr;QTimer poll;QJsonArray checks;bool failed=false,finished=false;int stage=0;
    QQuickItem*find(QQuickItem*at,const QString&name){if(at->objectName()==name)return at;for(auto child:at->childItems())if(auto item=find(child,name))return item;return nullptr;}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void field(const QString&name,const QString&value){if(auto item=find(window->contentItem(),name))item->setProperty("text",value);else{check("search field exists: "+name,false);finish();}}
    void open(){QMetaObject::invokeMethod(sheet,"openFrom",Q_ARG(QVariant,QRectF(300,82,600,37)));}
    void finish(){if(finished)return;finished=true;poll.stop();QJsonObject report{{"passed",!failed},{"checks",checks},{"syntheticData",true}};if(failed){auto card=find(window->contentItem(),"composer");report["diagnostics"]=QJsonObject{{"stage",stage},{"draftMode",mail->settings().value("draft_mode").toString()},{"composeState",mail->composer().value("state").toString()},{"loadedSession",card?card->property("loadedSession").toString():QString()},{"composerSession",mail->composer().value("session").toString()},{"shown",card&&card->property("shown").toBool()}};}if(auto path=qEnvironmentVariable("LUMA_SEARCH_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(report).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);mail->quit(failed?2:0);}
public:
    SearchSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        QTimer::singleShot(20000,this,[this]{if(!finished){check("search and mail-link flows finish within deadline",false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{mail->send("settingsUpdate",{{"draft_mode","manual"}});QTimer::singleShot(350,this,[this]{sheet=find(window->contentItem(),"searchSheet");check("custom search sheet loads",sheet);if(!sheet){finish();return;}open();check("search bar opens through a bounds morph",sheet->property("moving").toBool());QTimer::singleShot(750,this,[this]{auto card=find(window->contentItem(),"searchCard");check("search morph reaches its expanded bounds",card&&qAbs(card->width()-sheet->property("goalWidth").toReal())<.5&&!sheet->property("moving").toBool());sheet->setProperty("allAccounts",false);sheet->setProperty("selected",QVariantList{"preview-b"});field("searchFrom","Maya Chen");field("searchWords","launch");field("searchAfter","2026-01-01");field("searchBefore","2026-02-01");stage=1;QMetaObject::invokeMethod(sheet,"submit");});});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&data){if(kind=="error"){check("search/link command succeeds: "+data.value("message").toString(),false);finish();}});
        poll.setInterval(80);connect(&poll,&QTimer::timeout,this,[this]{if(finished)return;
            if(stage==1&&!sheet->property("shown").toBool()&&mail->search().contains("launch")&&mail->inbox()->rowCount()>1&&find(window->contentItem(),"mailSubject-"+mail->inbox()->at(1).value("key").toMap().value("id").toString())){
                auto subject=find(window->contentItem(),"mailSubject-"+mail->inbox()->at(1).value("key").toMap().value("id").toString());
                check("remote search terms are highlighted in actual conversation rows",subject->property("text").toString().contains("<b>launch</b>"));
                auto first=QJsonObject::fromVariantMap(mail->inbox()->at(0));
                mail->inbox()->patch(RecordModel::key(first),{{"subject","<img src='https://example.invalid'> launch & details"}});
                auto literal=find(window->contentItem(),"mailSubject-"+first.value("key").toObject().value("id").toString());
                const auto rendered=literal?literal->property("text").toString():QString();
                check("highlighting escapes untrusted mail headers in the real row",rendered.contains("&lt;img")&&!rendered.contains("<img")&&rendered.contains("&amp; details")&&rendered.contains("<b>launch</b>"));
                stage=2;check("advanced search builds validated Gmail operators",mail->search().contains("from:\"Maya Chen\"")&&mail->search().contains("after:2026/01/01")&&mail->search().contains("before:2026/02/01"));check("search retains the selected account scope",!mail->searchAllAccounts()&&mail->searchAccounts()==QVariantList{"preview-b"}&&mail->account()=="preview-b");mail->refreshView();QTimer::singleShot(300,this,[this]{check("manual refresh keeps the search subset",mail->searchAccounts()==QVariantList{"preview-b"}&&mail->search().contains("launch"));mail->searchCurrent("newer");QTimer::singleShot(200,this,[this]{check("editing the raw query keeps its account scope",mail->searchAccounts()==QVariantList{"preview-b"}&&mail->search()=="newer");open();field("searchWords","do-not-apply");QMetaObject::invokeMethod(sheet,"submit");QMetaObject::invokeMethod(sheet,"close");stage=3;QTimer::singleShot(600,this,[this]{check("cancelling a pending builder does not replace the query",mail->search()=="newer");stage=4;mail->openLink("mailto:pat+tag@example.com?subject=Qt%20test&body=From%20a%20link&bcc=copy@example.com");});});});
            }else if(stage==4&&!mail->composer().isEmpty()){
                stage=5;QTimer::singleShot(750,this,[this]{auto card=find(window->contentItem(),"composer"),to=find(window->contentItem(),"composeTo"),bcc=find(window->contentItem(),"composeBcc");const auto state=mail->composer();check("mail links use the currently selected sending account",state["account"]=="preview-b");check("mail-link fields reach the real composer",to&&to->property("text").toString()=="pat+tag@example.com"&&state["subject"]=="Qt test");check("prefilled Bcc remains visible for review",bcc&&bcc->isVisible()&&bcc->property("text").toString()=="copy@example.com");check("mail links start with the compact composer",card&&!card->property("expanded").toBool());if(auto path=qEnvironmentVariable("LUMA_SEARCH_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);if(card)QMetaObject::invokeMethod(card,"requestClose");stage=6;});
            }else if(stage==6&&mail->composer().isEmpty()){
                const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");check("search and mail links create no local mail or draft archive",!QFileInfo::exists(root+"/config/composer")&&!QFileInfo::exists(root+"/cache/bodies"));finish();
            }
        });poll.start();
    }
};
void startSearchSmoke(Bridge*m,QQuickWindow*w){new SearchSmoke(m,w);}
