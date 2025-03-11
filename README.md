# JLPT [非公式] 日本語能力試験対策学習アプリ
日本語検定アプリのバックグラウンド API部


[紹介動画](https://www.youtube.com/watch?v=I4o_v7d3yR8)
[![紹介動画](https://img.youtube.com/vi/I4o_v7d3yR8/maxresdefault.jpg)](https://www.youtube.com/watch?v=I4o_v7d3yR8)

[JLPT非公式日本語能力試験対策学習アプリ](https://jlpt.howlrs.net/)

## Deploy to Google cloud run
```sh
$ gcloud run deploy backend --source .
```


## [TODO] FEATURES
- レベル・カテゴリを入力（選択）すると問題を生成する
  - 作問エンドポイント
  - Gemini作問（Functions）
  - 型にバインド
  - 返す
  - DB保存
- Activityなどのユーザ情報の保存
  - 国・言語、最後のログイン、回答履歴、
  - データテーブルまたはグラフ化
  - 適宜広告