#include "bridge.h"
#include "rich_document.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QJsonDocument>
#include <QSaveFile>
#include <QFileInfo>
class SignatureSmoke:public QObject {
    Bridge*mail;QQuickWindow*window;QQuickItem*sheet=nullptr;RichDocument*rich=nullptr;QJsonArray checks;bool failed=false,finished=false;int stage=0;QString session;
    QQuickItem*find(QQuickItem*at,const QString&name){if(at->objectName()==name)return at;for(auto child:at->childItems())if(auto item=find(child,name))return item;return nullptr;}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void finish(){if(finished)return;finished=true;QJsonObject report{{"passed",!failed},{"checks",checks},{"syntheticData",true},{"note","Actual native editor and engine; preview signatures save only in RAM and never call Gmail."}};if(auto path=qEnvironmentVariable("LUMA_SIGNATURE_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(report).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);if(sheet&&!sheet->property("session").toString().isEmpty())QMetaObject::invokeMethod(sheet,"discard");QTimer::singleShot(150,this,[this]{mail->quit(failed?2:0);});}
    void invoke(const char*method){QMetaObject::invokeMethod(sheet,method);}
public:
    SignatureSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        QTimer::singleShot(22000,this,[this]{if(!finished){check("signature workflows finish within deadline",false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{sheet=find(window->contentItem(),"signatureSheet");check("signature editor stays lazy before opening",sheet&&!sheet->property("opened").toBool()&&!window->findChild<RichDocument*>("signatureDocument"));mail->send("signatureOpen",{{"account","preview"}});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&data){if(finished)return;
            if(kind=="error"){check("signature command succeeds: "+data.value("message").toString(),false);finish();return;}
            if(kind=="signatureOpened"&&stage==0){stage=1;session=data["session"].toString();QTimer::singleShot(800,this,[this]{rich=window->findChild<RichDocument*>("signatureDocument");check("online signature loads the actual native document",rich&&rich->nativeDocument()->toPlainText().contains("Alex Morgan"));check("existing source begins read-only and unchanged",rich&&rich->locked()&&!sheet->property("dirty").toBool()&&!sheet->property("visual").toBool());if(!rich){finish();return;}sheet->setProperty("visual",true);emit mail->signaturePictureChosen(session,"never-open-this-file.png","sample.png");auto approval=window->findChild<QObject*>("signaturePublicQuestion");check("local picture upload asks for explicit public-link approval",approval&&approval->property("visible").toBool());if(approval)approval->setProperty("opened",false);rich->selection(0,4,4);rich->foreground(QColor("#123abc"));check("signature has selected text colours and rich formatting",rich->html().contains("#123abc"));const int end=rich->nativeDocument()->characterCount()-1;rich->selection(end,end,end);check("hosted pictures keep HTTPS sources",rich->picture("https://example.com/mark.png")&&!rich->picture("file:///private"));mail->send("signaturePictures",{{"session",session},{"html",rich->html()}});});}
            else if(kind=="signaturePicture"&&stage==1){check("explicit picture loading supplies bounded in-memory pixels",!data["bytes"].toString().isEmpty()&&rich->nativeDocument()->resource(QTextDocument::ImageResource,QUrl(data["url"].toString())).isValid());}
            else if(kind=="signatureStatus"&&stage==1&&!data["busy"].toBool()){stage=2;QTimer::singleShot(100,this,[this]{const int end=rich->nativeDocument()->characterCount()-1;rich->selection(end,end,end);check("signature tables use native rich text",rich->table(2,2)&&rich->html().contains("<table"));if(auto path=qEnvironmentVariable("LUMA_SIGNATURE_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);QMetaObject::invokeMethod(sheet,"save",Q_ARG(QVariant,false));});}
            else if(kind=="signatureSaved"&&stage==2){stage=3;check("preview save sanitizes HTML without a local signature store",data["preview"].toBool()&&data["html"].toString().contains("#123abc")&&!data["html"].toString().contains("data:image"));check("confirmed save clears the editor dirty state",!sheet->property("dirty").toBool());}
            else if(kind=="signatureStatus"&&stage==3&&!data["busy"].toBool()){stage=4;QTimer::singleShot(100,this,[this]{QTextCursor cursor(rich->nativeDocument());cursor.movePosition(QTextCursor::End);cursor.insertText("Unsaved change");mail->quit();});}
            else if(kind=="signatureCloseQuestion"&&stage==4){stage=5;check("app quit asks about unsaved online signature edits",data["quit"].toBool());invoke("keepEditing");}
            else if(kind=="quitCancelled"&&stage==5){stage=6;check("Keep editing cancels application quit",sheet->property("opened").toBool());invoke("requestClose");}
            else if(kind=="signatureCloseQuestion"&&stage==6){stage=7;check("normal close asks before discarding signature edits",!data["quit"].toBool());invoke("discard");}
            else if(kind=="signatureClosed"&&stage==7){stage=8;QTimer::singleShot(900,this,[this]{check("closing releases the signature document and previews",!window->findChild<RichDocument*>("signatureDocument")&&sheet->property("session").toString().isEmpty());const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");check("signature editing creates no mail, image or local signature archive",!QFileInfo::exists(root+"/config/signatures.json")&&!QFileInfo::exists(root+"/config/composer")&&!QFileInfo::exists(root+"/cache/bodies"));finish();});}
        });
    }
};
void startSignatureSmoke(Bridge*m,QQuickWindow*w){new SignatureSmoke(m,w);}
