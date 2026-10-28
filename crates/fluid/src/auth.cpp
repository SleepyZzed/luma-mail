#include "bridge.h"
#include <QApplication>
#include <QClipboard>
static bool googleAuthUrl(const QUrl&url){return url.isValid()&&url.scheme()=="https"&&url.host()=="accounts.google.com"&&url.userInfo().isEmpty()&&(url.port()==-1||url.port()==443)&&url.path()=="/o/oauth2/v2/auth"&&!url.hasFragment()&&url.toEncoded().size()<=16384;}
void Bridge::openAuthBrowser(){
    const QUrl url(m_authState.value("url").toString());if(!authRequest||m_authState.value("phase")!="waiting"||!googleAuthUrl(url))return;
    const bool opened=authOpener(url);m_authState["browserOpened"]=opened;m_authState["message"]=opened?"Finish sign-in in your browser. Return here when Google confirms your choice.":"Your browser could not be opened. Try again or copy the sign-in link into your browser.";emit authChanged();
}
void Bridge::copyAuthLink(){
    const auto source=m_authState.value("url").toString();if(!authRequest||m_authState.value("phase")!="waiting"||!googleAuthUrl(QUrl(source)))return;
    if(clipboardFixture)clipboardFixture->setText(source);else QApplication::clipboard()->setText(source);m_authState["message"]="Sign-in link copied. Paste it into your browser to continue.";emit authChanged();
}
void Bridge::cancelAuth(){
    if(!authRequest){m_authState.clear();emit authChanged();return;}
    if(m_authState.value("phase")=="finishing")return;
    m_authState["phase"]="cancelling";m_authState["message"]="Closing this sign-in request";emit authChanged();send("cancel",{{"request",qint64(authRequest)}});
}
void Bridge::retryAuth(){if(!authRequest)send("authStart",authOptions);}
bool Bridge::authEvent(const QString&kind,const QJsonObject&data,quint64 id){
    const bool event=kind=="authUrl"||kind=="authFinishing"||kind=="authComplete"||kind=="authCancelled";
    if(kind=="error"&&cancelledAuth.contains(id))return true;
    if(!event&&!(kind=="error"&&authRequest&&id==authRequest))return false;
    if(!authRequest||id!=authRequest)return true;
    if(kind=="authUrl"){
        if(m_authState.value("phase")=="cancelling")return true;
        const auto source=data["url"].toString();const QUrl url(source);
        if(!googleAuthUrl(url)){const auto cancelled=authRequest;cancelAuth();cancelledAuth.append(cancelled);while(cancelledAuth.size()>8)cancelledAuth.removeFirst();authRequest=0;m_authState={{"phase","error"},{"message","Google returned an invalid sign-in address. Start sign-in again."}};emit authChanged();return true;}
        if(m_authState.value("url")==source&&m_authState.value("phase")=="waiting")return true;
        m_authState["url"]=source;m_authState["phase"]="waiting";openAuthBrowser();return true;
    }
    if(kind=="authFinishing"){m_authState["phase"]="finishing";m_authState["url"]="";m_authState["message"]="Saving the connection securely. Please wait.";}
    else if(kind=="authComplete"){authRequest=0;m_authState={{"phase","complete"},{"message","Google account connected"}};}
    else if(kind=="authCancelled"){cancelledAuth.append(authRequest);while(cancelledAuth.size()>8)cancelledAuth.removeFirst();authRequest=0;m_authState={{"phase","cancelled"}};}
    else{authRequest=0;m_authState={{"phase","error"},{"message",data["message"].toString()}};}
    emit authChanged();return true;
}
