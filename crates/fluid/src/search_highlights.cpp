#include "search_highlights.h"
#include <QStringList>
#include <algorithm>

namespace {
QString escaped(const QString&value){auto result=value.toHtmlEscaped();result.replace('\n',"<br>");return result;}
QStringList terms(const QString&query){
    if(query.size()>8192||query.count('"')%2)return {};
    QStringList tokens,words;QString current;bool quoted=false;
    for(const auto character:query){
        if(character=='"')quoted=!quoted;
        if(character.isSpace()&&!quoted){if(!current.isEmpty()){tokens.append(current);current.clear();}}
        else current+=character;
    }
    if(!current.isEmpty())tokens.append(current);
    for(auto token:tokens){
        if(token.startsWith('-')||token=="OR"||token=="AND")continue;
        if(token.startsWith('+'))token.remove(0,1);
        // Do not colour addresses, labels, dates or other operator values as
        // body terms. Complex grouping remains Gmail's responsibility.
        if(token.startsWith("subject:",Qt::CaseInsensitive))token.remove(0,8);
        else if(token.contains(':'))continue;
        const bool phrase=token.startsWith('"')&&token.endsWith('"');
        if(phrase)token=token.mid(1,token.size()-2);
        else if(token.contains('(')||token.contains(')')||token.contains('{')||token.contains('}')||token.contains('*'))continue;
        if(token.size()<2||token.size()>64||token.contains('"'))continue;
        if(std::none_of(words.begin(),words.end(),[&](const auto&word){return word.compare(token,Qt::CaseInsensitive)==0;}))words.append(token);
        if(words.size()==8)break;
    }
    std::stable_sort(words.begin(),words.end(),[](const auto&a,const auto&b){return a.size()>b.size();});
    return words;
}
}
QString SearchHighlights::render(const QString&input,const QString&query,const QColor&colour)const{
    if(!initialized||previous!=query){
        initialized=true;previous=query;QStringList patterns;for(const auto&term:terms(query))patterns.append(QRegularExpression::escape(term));
        expression=QRegularExpression(patterns.join('|'),QRegularExpression::CaseInsensitiveOption|QRegularExpression::UseUnicodePropertiesOption);
    }
    auto plain=input.left(8192);if(plain.size()&&plain.back().isHighSurrogate())plain.chop(1);if(input.size()>plain.size())plain+=QChar(0x2026);
    if(expression.pattern().isEmpty()||!expression.isValid())return escaped(plain);
    const auto name=(colour.isValid()?colour:QColor("#7160a3")).name(QColor::HexRgb);
    QString output;qsizetype at=0;int count=0;auto matches=expression.globalMatch(plain);
    while(matches.hasNext()&&count++<32){const auto match=matches.next();output+=escaped(plain.mid(at,match.capturedStart()-at));output+="<font color=\""+name+"\"><b>"+escaped(match.captured())+"</b></font>";at=match.capturedEnd();}
    output+=escaped(plain.mid(at));return output;
}
