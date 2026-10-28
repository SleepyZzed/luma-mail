#include "bridge.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QJsonDocument>
#include <QSaveFile>
#include <QFileInfo>
class AuthSmoke:public QObject {
    Q_OBJECT
    Bridge*mail;QQuickWindow*window;QJsonArray checks;QStringList links;quint64 cancelled=0;int stage=0;bool failed=false,finished=false;
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void start(){mail->send("authStart",{{"drive",true},{"settings",true},{"photos",true}});}
    void finish(){if(finished)return;finished=true;QJsonObject result{{"passed",!failed},{"checks",checks},{"syntheticData",true},{"note","Real private-pipe authorization state machine; browser and clipboard are isolated, with no Google access, vault write or real consent."}};if(auto path=qEnvironmentVariable("LUMA_AUTH_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(result).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(result).toJson(QJsonDocument::Compact);mail->clipboardFixture.clear();mail->quit(failed?2:0);}
public:
    AuthSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        auto clipboard=new QMimeData;clipboard->setParent(this);mail->clipboardFixture=clipboard;
        mail->authOpener=[this](const QUrl&url){links.append(url.toString());return links.size()>1;};
        QTimer::singleShot(16000,this,[this]{if(!finished){check("sign-in recovery completes before its deadline",false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(300,this,[this]{
            mail->m_inbox.replace({});mail->m_accounts.reconcileOrdered({});mail->m_settings["accounts"]=QVariantList{};emit mail->settingsChanged();
            QTimer::singleShot(150,this,[this]{auto action=window->findChild<QQuickItem*>("onboardingGoogleSignIn");check("an empty account list offers one-click Google onboarding",action&&action->isVisible());if(action)QMetaObject::invokeMethod(action,"clicked");else finish();});
        });});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&){
            if(finished)return;
            if(kind=="error"||kind=="authError"){check("synthetic auth commands have no unexpected errors",false);finish();return;}
            if(kind=="authUrl"&&stage==0){stage=1;const auto state=mail->authState();check("browser failure is visible without losing the sign-in request",state["phase"]=="waiting"&&!state["browserOpened"].toBool()&&!state["url"].toString().isEmpty());check("one sign-in requests Gmail and its optional feature connections",mail->authOptions["drive"].toBool()&&mail->authOptions["photos"].toBool()&&mail->authOptions["settings"].toBool());mail->copyAuthLink();check("explicit Copy uses only the active sign-in URL",mail->clipboardFixture->text()==state["url"].toString());mail->openAuthBrowser();check("browser retry uses the same request URL",links.size()==2&&links[0]==links[1]&&mail->authState()["browserOpened"].toBool());QTimer::singleShot(650,this,[this]{auto sheet=window->findChild<QQuickItem*>("authPanel");check("auth controls appear in the custom spring sheet",sheet&&sheet->property("opened").toBool()&&sheet->isVisible());cancelled=mail->authRequest;mail->cancelAuth();});}
            else if(kind=="authCancelled"&&stage==1){stage=2;check("Cancel clears the URL and active request",!mail->authRequest&&mail->authState()["url"].toString().isEmpty());QTimer::singleShot(750,this,[this]{auto sheet=window->findChild<QQuickItem*>("authPanel");check("cancelled sign-in slides closed",sheet&&!sheet->isVisible());start();});}
            else if(kind=="authUrl"&&stage==2){stage=3;const auto before=links.size();mail->receive({{"id",qint64(cancelled)},{"event","authUrl"},{"data",QJsonObject{{"url","https://accounts.google.com/o/oauth2/v2/auth?client_id=old"}}}});check("late cancelled callbacks cannot reopen the browser",links.size()==before&&mail->authRequest!=cancelled);mail->send("previewAuthApprove",{{"request",qint64(mail->authRequest)}});}
            else if(kind=="authFinishing"&&stage==3){stage=4;const auto request=mail->authRequest;mail->cancelAuth();check("credential commit disables cancellation and clears its browser URL",mail->authState()["phase"]=="finishing"&&mail->authState()["url"].toString().isEmpty()&&mail->authRequest==request);mail->send("cancel",{{"request",qint64(request)}});}
            else if(kind=="authComplete"&&stage==4){stage=5;check("an accepted commit completes despite a late Cancel",mail->authState()["phase"]=="complete"&&!mail->authRequest&&mail->authState()["url"].toString().isEmpty());QTimer::singleShot(1700,this,[this]{auto sheet=window->findChild<QQuickItem*>("authPanel");check("successful connection morphs and releases its sheet",sheet&&!sheet->isVisible());const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");check("sign-in recovery creates no local mailbox, draft or credential file",!QFileInfo::exists(root+"/cache/bodies")&&!QFileInfo::exists(root+"/config/composer")&&!QFileInfo::exists(root+"/config/credentials.json"));start();});}
            else if(kind=="authUrl"&&stage==5){stage=6;const auto before=links.size();mail->receive({{"id",qint64(mail->authRequest)},{"event","authUrl"},{"data",QJsonObject{{"url","javascript:alert(1)"}}}});check("non-Google and active-content sign-in links never reach the browser",links.size()==before&&mail->authState()["phase"]=="error"&&!mail->authRequest);mail->cancelAuth();QTimer::singleShot(750,this,[this]{finish();});}
        });
    }
};
void startAuthSmoke(Bridge*m,QQuickWindow*w){new AuthSmoke(m,w);}
#include "auth_smoke.moc"
