#include "record_model.h"
#include <QSet>
QVariant RecordModel::data(const QModelIndex&i,int role)const{return i.isValid()&&i.row()<rows.size()&&role==Qt::UserRole+1?QVariant(rows[i.row()].toVariantMap()):QVariant{};}
QString RecordModel::key(const QJsonObject&r){auto k=r["key"].toObject();return k.isEmpty()?r["id"].toString():k["account"].toString()+QChar(0)+k["id"].toString();}
void RecordModel::reindex(){positions.clear();for(int i=0;i<rows.size();++i)positions.insert(key(rows[i]),i);}
int RecordModel::indexFor(const QString&key)const{return positions.value(key,-1);}
void RecordModel::replace(const QJsonArray&a){beginResetModel();rows.clear();for(auto v:a){if(rows.size()==1000)break;rows.append(v.toObject());}reindex();endResetModel();emit countChanged();}
void RecordModel::reconcileOrdered(const QJsonArray&items){
    QList<QJsonObject>wanted;QSet<QString>keys;for(const auto&value:items){auto row=value.toObject();auto id=key(row);if(id.isEmpty()||keys.contains(id))continue;keys.insert(id);wanted.append(row);if(wanted.size()==1000)break;}
    const auto before=rows.size();
    for(int i=rows.size()-1;i>=0;--i)if(!keys.contains(key(rows[i]))){beginRemoveRows({},i,i);rows.removeAt(i);reindex();endRemoveRows();}
    reindex();
    for(int i=0;i<wanted.size();++i){const auto at=indexFor(key(wanted[i]));
        if(at<0){beginInsertRows({},i,i);rows.insert(i,wanted[i]);reindex();endInsertRows();}
        else{if(at!=i){beginMoveRows({},at,at,{},at>i?i:i+1);rows.move(at,i);reindex();endMoveRows();}if(rows[i]!=wanted[i]){rows[i]=wanted[i];emit dataChanged(index(i),index(i),{Qt::UserRole+1});}}
    }
    if(rows.size()!=before)emit countChanged();
}
void RecordModel::append(const QJsonArray&a){QList<QJsonObject>fresh;QSet<QString>seen;for(auto&r:rows)seen.insert(key(r));for(auto v:a){auto r=v.toObject();auto k=key(r);if(!seen.contains(k)&&rows.size()+fresh.size()<1000){fresh.append(r);seen.insert(k);}}if(fresh.isEmpty())return;beginInsertRows({},rows.size(),rows.size()+fresh.size()-1);rows.append(fresh);reindex();endInsertRows();emit countChanged();}
void RecordModel::update(const QJsonObject&r,bool insert){
    const auto k=key(r);const auto existing=indexFor(k);int at=0;
    for(int i=0;i<rows.size();++i)if(i!=existing&&rows[i]["date"].toInteger()>r["date"].toInteger())++at;
    if(existing>=0){rows[existing]=r;if(at!=existing){beginMoveRows({},existing,existing,{},at>existing?at+1:at);rows.move(existing,at);reindex();endMoveRows();}emit dataChanged(index(at),index(at),{Qt::UserRole+1});return;}
    if(!insert||at>=1000)return;
    if(rows.size()>=1000){beginRemoveRows({},rows.size()-1,rows.size()-1);rows.removeLast();reindex();endRemoveRows();}
    beginInsertRows({},at,at);rows.insert(at,r);reindex();endInsertRows();emit countChanged();
}
void RecordModel::removeKeys(const QJsonArray&a){QSet<QString>keys;for(auto k:a){QJsonObject r;r["key"]=k;keys.insert(key(r));}for(int i=rows.size()-1;i>=0;--i)if(keys.contains(key(rows[i]))){beginRemoveRows({},i,i);rows.removeAt(i);reindex();endRemoveRows();}emit countChanged();}
void RecordModel::patch(const QString&id,const QJsonObject&v){const auto i=indexFor(id);if(i<0)return;for(auto it=v.begin();it!=v.end();++it)rows[i][it.key()]=it.value();emit dataChanged(index(i),index(i),{Qt::UserRole+1});}
void RecordModel::windowMessages(const QJsonArray&items,bool arrivals){
    QJsonArray merged;for(const auto&value:items){const auto header=value.toObject();const auto i=indexFor(header["id"].toString());auto row=i>=0?rows[i]:QJsonObject{};for(auto at=header.begin();at!=header.end();++at)row[at.key()]=at.value();if(i<0)row["arrived"]=arrivals;merged.append(row);if(merged.size()==32)break;}reconcileOrdered(merged);
}
void RecordModel::mergeMessages(const QJsonArray&a){for(auto value:a){auto r=value.toObject();auto id=r["id"].toString();if(indexFor(id)>=0)patch(id,r);else{r["arrived"]=true;append({r});}}}
void RecordModel::trimBodies(const QString&keep){
    qint64 bytes=0;int count=0;for(const auto&r:rows){bytes+=(r["text"].toString().size()+r["document"].toString().size())*2;count+=r.contains("text");}
    for(int i=0;i<rows.size()&&(bytes>16*1024*1024||count>8);++i){auto&r=rows[i];if(r["id"]==keep||!r.contains("text"))continue;bytes-=(r["text"].toString().size()+r["document"].toString().size())*2;--count;r.remove("text");r.remove("document");r.remove("links");r.remove("bodyLoaded");emit dataChanged(index(i),index(i),{Qt::UserRole+1});}
}

