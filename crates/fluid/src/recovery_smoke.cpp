#include "bridge.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QSaveFile>
#include <QJsonDocument>
class RecoverySmoke:public QObject {
    Bridge*mail;QQuickWindow*window;QJsonArray checks;int stage=0;bool failed=false,finished=false;
    QQuickItem*find(QQuickItem*item,const QString&name){if(item->objectName()==name)return item;for(auto child:item->childItems())if(auto result=find(child,name))return result;return nullptr;}
    void check(const QString&name,bool value){checks.append(QJsonObject{{"check",name},{"passed",value}});failed|=!value;}
    void finish(){if(finished)return;finished=true;QJsonObject report{{"passed",!failed},{"checks",checks},{"syntheticData",true}};if(auto path=qEnvironmentVariable("LUMA_RECOVERY_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(report).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);mail->quit(failed?2:0);}
    void retry(const QString&name){auto button=find(window->contentItem(),name);check("native retry control is visible: "+name,button&&button->isVisible()&&button->isEnabled());if(!button){finish();return;}check("native retry action is wired: "+name,QMetaObject::invokeMethod(button,"clicked"));}
public:
    RecoverySmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        QTimer::singleShot(20000,this,[this]{if(!finished){check("reader recovery completes within deadline",false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(400,this,[this]{mail->openThread("preview","preview0");});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&){if(finished)return;
            if(kind=="error"){check("expected read failures stay inside the reading pane",false);finish();return;}
            if(kind=="readerError"&&stage<3){const int failure=stage++;QTimer::singleShot(700,this,[this,failure]{
                check("read failure preserves the loaded inbox at stage "+QString::number(failure),mail->readerOpen()&&mail->inbox()->rowCount()==8&&mail->error().isEmpty());
                if(failure==0){check("metadata error is actionable",!mail->reader().value("error").toString().isEmpty());retry("retryReader");}
                else if(failure==1){const auto message=mail->messages()->at(0);check("body failure clears its pending state",!message.value("bodyPending").toBool()&&!message.value("bodyError").toString().isEmpty());retry("retryBody-preview0");}
                else {const auto message=mail->messages()->at(0);check("HTML failure retains the loaded body and clears its pending state",message.value("bodyLoaded").toBool()&&!message.value("documentPending").toBool()&&!message.value("documentError").toString().isEmpty());retry("retryDocument-preview0");}
            });}
            else if(kind=="readerDocument"&&stage==3){stage=4;QTimer::singleShot(1000,this,[this]{const auto message=mail->messages()->at(0);check("retry restores the HTML document in the same reading pane",!message.value("document").toString().isEmpty()&&message.value("documentError").toString().isEmpty()&&!message.value("documentPending").toBool());check("successful retry retains authored server content",message.value("text").toString().contains("synthetic")&&mail->reader().value("key").toMap()["id"]=="preview0");mail->closeReader();check("closing releases recovered bodies and errors",mail->messages()->rowCount()==0&&mail->reader().isEmpty());finish();});}
        });
    }
};
void startRecoverySmoke(Bridge*m,QQuickWindow*w){new RecoverySmoke(m,w);}
