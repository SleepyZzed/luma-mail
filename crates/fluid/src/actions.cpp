#include "bridge.h"
#include <QRegularExpression>
#include <QUrl>
QVariantMap Bridge::actionState(const QVariantMap&requested)const{
    QVariantList rows;
    if(!m_selection.isEmpty()){for(int i=0;i<m_inbox.rowCount();++i){auto row=m_inbox.at(i);if(isSelected(row))rows.append(row);}}
    else{
        auto row=requested;const auto key=row.isEmpty()?m_reader.value("key").toMap():row.value("key").toMap();
        const int index=m_inbox.indexFor(key.value("account").toString()+QChar(0)+key.value("id").toString());
        if(index>=0)row=m_inbox.at(index);else if(row.isEmpty()&&key==m_openedRow.value("key").toMap())row=m_openedRow;
        if(!row.isEmpty())rows.append(row);
    }
    QVariantList keys;bool unread=false,starred=!rows.isEmpty(),important=!rows.isEmpty();
    for(const auto&value:rows){auto row=value.toMap();keys.append(row["key"]);unread|=row.value("unread").toBool();starred&=row.value("starred").toBool();important&=row.value("important").toBool();}
    const auto first=rows.isEmpty()?QVariantMap():rows.first().toMap();const auto key=first.value("key").toMap();
    const auto sender=first.value("sender").toString();const auto match=QRegularExpression("<([^<>]+)>").match(sender);const auto address=(match.hasMatch()?match.captured(1):sender).trimmed().toLower();bool connected=m_preview,blocked=false;
    for(const auto&value:m_settings.value("accounts").toList()){const auto account=value.toMap();if(account["id"]!=key["account"])continue;connected|=account.value("sender_controls").toBool();for(const auto&entry:account.value("blocked_senders").toList())blocked|=entry.toMap().value("address").toString()==address;}
    return {{"count",keys.size()},{"keys",keys},{"key",key},{"account",key.value("account")},{"sender",sender},{"address",address},{"ruleConnected",connected},{"blocked",blocked},{"unread",unread},{"starred",starred},{"important",important}};
}
void Bridge::composeFor(const QString&mode,const QVariantMap&key){if(m_selection.size()>1||key.isEmpty())return;send("composeStart",{{"mode",mode},{"key",key},{"account",key.value("account")}});}
void Bridge::mutateMessage(const QString&action,const QVariantMap&message){const auto key=m_reader.value("key").toMap();if(m_selection.size()>1||key.isEmpty()||message.value("id").toString().isEmpty()||m_messages.indexFor(message.value("id").toString())<0)return;send("mutate",{{"keys",QVariantList{key}},{"message",message.value("id")},{"action",action}});}
void Bridge::handleMailto(const QUrl&uri){queueMailto(uri.toString(QUrl::FullyEncoded));}
void Bridge::releaseComposerView(const QString&session){if(m_composer.value("session").toString()==session&&(m_composer.value("state")=="sent"||m_composer.value("state")=="closed")){m_composer.clear();emit composerChanged();}}
void Bridge::queueMailto(const QString&uri){
    if(!uri.startsWith("mailto:",Qt::CaseInsensitive)||uri.toUtf8().size()>65536){m_error="Invalid or oversized email link.";emit errorChanged();return;}
    if(!pendingMailto.isEmpty()&&pendingMailto!=uri){m_error="Another email link is already waiting. Open it before trying a second link.";emit errorChanged();return;}
    pendingMailto=uri;emit mailLinkWaitingChanged();emit showRequested();flushMailto();
}
void Bridge::flushMailto(){
    if(!engineReady||pendingMailto.isEmpty())return;
    bool connected=false;for(const auto&value:m_settings.value("accounts").toList())connected|=value.toMap().value("enabled").toBool();if(!connected)return;
    const auto uri=pendingMailto;pendingMailto.clear();emit mailLinkWaitingChanged();send("composeStart",{{"mode","new"},{"mailto",uri},{"account",m_account}});
}
