// Native synthetic smoke through the real QML form, bridge and Rust child.
#include "bridge.h"
#include "rich_document.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QKeyEvent>
#include <QJsonDocument>
#include <QSaveFile>
#include <QTimer>
class ComposerSmoke:public QObject {
    Bridge *mail;QQuickWindow *window;QQuickItem *card=nullptr;RichDocument *document=nullptr;
    int stage=0;bool failed=false,finished=false;QString session;QJsonArray checks;
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void invoke(const char*method){QMetaObject::invokeMethod(card,method);}
    void key(int code,Qt::KeyboardModifiers modifiers=Qt::NoModifier){QKeyEvent press(QEvent::KeyPress,code,modifiers);QCoreApplication::sendEvent(window,&press);QKeyEvent release(QEvent::KeyRelease,code,modifiers);QCoreApplication::sendEvent(window,&release);}
    void finish(){if(finished)return;finished=true;QJsonObject r{{"passed",!failed},{"checks",checks},{"syntheticData",true}};if(auto path=qEnvironmentVariable("LUMA_COMPOSER_REPORT");!path.isEmpty()){QSaveFile f(path);if(f.open(QIODevice::WriteOnly)){f.write(QJsonDocument(r).toJson());f.commit();}}qInfo().noquote()<<QJsonDocument(r).toJson(QJsonDocument::Compact);mail->send("settingsUpdate",{{"draft_mode","manual"}});mail->quit(failed?2:0);}
public:
    ComposerSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(300,this,[this]{mail->requestCompose();});});
        connect(mail,&Bridge::received,this,[this](const QString&event,const QVariantMap&data){
            if(finished)return;
            if(event=="error"){check("no unexpected composer error: "+data.value("message").toString(),false);stage=99;finish();return;}
            if(event=="composeOpened"&&stage==0){
                stage=1;session=data["session"].toString();card=window->findChild<QQuickItem*>("composer");document=window->findChild<RichDocument*>("composeDocument");
                check("engine session loads the native form",card&&document&&!session.isEmpty());if(!card||!document){finish();return;}
                QTimer::singleShot(750,this,[this]{
                    auto to=window->findChild<QQuickItem*>("composeTo"),subject=window->findChild<QQuickItem*>("composeSubject"),body=window->findChild<QQuickItem*>("composeBody");
                    check("new compose focuses To",window->activeFocusItem()==to);key(Qt::Key_Tab);check("Tab skips formatting controls to Subject",window->activeFocusItem()==subject);key(Qt::Key_Tab);check("Tab from Subject enters body",window->activeFocusItem()==body);key(Qt::Key_Backtab,Qt::ShiftModifier);check("Shift Tab returns to Subject",window->activeFocusItem()==subject);
                    to->setProperty("text","person@example.com");subject->setProperty("text","Synthetic composer test");QTextCursor c(document->nativeDocument());c.insertText("Hello synthetic world");document->selection(0,5,5);document->toggle("bold");
                    check("selected formatting produces rich outgoing markup",document->payload()["html"].toBool()&&document->html().contains("font-weight:700"));
                    if(auto path=qEnvironmentVariable("LUMA_COMPOSER_SCREENSHOT");!path.isEmpty())window->grabWindow().save(path);
                    invoke("sync");invoke("requestClose");
                });
            }else if(event=="composeCloseQuestion"&&stage==1){
                stage=2;check("close asks before any Gmail draft write",data["revision"].toULongLong()>0);
                QMetaObject::invokeMethod(card,"answerClose",Q_ARG(QVariant,QString("keep")));
                QTimer::singleShot(200,this,[this]{mail->requestCompose("reply");});
            }else if(event=="composeFocus"&&stage==2){
                stage=3;check("repeated compose focuses the same session",data["session"]==session&&!card->property("expanded").toBool());mail->quit();
            }else if(event=="composeCloseQuestion"&&stage==3){
                stage=4;check("quit uses the same draft close policy",data["session"]==session);QMetaObject::invokeMethod(card,"answerClose",Q_ARG(QVariant,QString("keep")));
            }else if(event=="quitCancelled"&&stage==4){
                stage=5;check("keep editing cancels quit",card->property("shown").toBool());mail->send("settingsUpdate",{{"draft_mode","manual"}});
            }else if(event=="settings"&&stage==5){stage=6;invoke("requestClose");}
            else if(event=="composeClosed"&&stage==6){stage=7;check("manual close clears engine content",data["quote"].toString().isEmpty()&&data["attachments"].toList().isEmpty());QTimer::singleShot(900,this,[this]{check("close clears the native rich document",document->nativeDocument()->isEmpty());mail->selectRow(0,0,240,100,450,68);});}
            else if(event=="body"&&stage==7){stage=8;mail->requestCompose("reply",mail->messages()->at(0));}
            else if(event=="composeOpened"&&stage==8){
                stage=9;session=data["session"].toString();check("reply has read-only history outside the editor",!data["quote"].toString().isEmpty()&&document->nativeDocument()->isEmpty());
                QTimer::singleShot(750,this,[this]{check("reply docks inside the reading thread",card->property("inlineMode").toBool()&&!card->property("expanded").toBool());check("reply focuses authored body",window->activeFocusItem()==window->findChild<QQuickItem*>("composeBody"));mail->requestCompose("reply",mail->messages()->at(0));});
            }else if(event=="composeFocus"&&stage==9){stage=10;check("repeated reply never maximizes or creates a draft",data["session"]==session&&card->property("inlineMode").toBool()&&!card->property("expanded").toBool());invoke("requestClose");}
            else if(event=="composeClosed"&&stage==10){stage=11;QTimer::singleShot(900,this,[this]{check("inline close collapses and releases the surface",!card->isVisible());finish();});}
        });
        QTimer::singleShot(18000,this,[this]{if(!finished){check("composer smoke completes within deadline",false);finish();}});
    }
};
void startComposerSmoke(Bridge*m,QQuickWindow*w){new ComposerSmoke(m,w);}
