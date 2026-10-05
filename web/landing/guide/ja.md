# mskill · Agent Skills をプロジェクト間で再利用

mskill でローカルの Skill ライブラリを管理。Agent Skills の取得、公開、コピーとリンク。ポータブルな .skill パッケージで最新の内容を共有。

## CLI をインストール

アーカイブを展開し、mskill（Windows は mskill.exe）を PATH に追加してください。ライセンスを同梱し、チェックサムも配布しています。

- [Windows x86_64](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-windows-x86_64.tar.gz)
- [Linux x86_64](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-linux-x86_64.tar.gz)
- [macOS Apple Silicon](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-macos-aarch64.tar.gz)

[すべてのリリースとチェックサム](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest)

## ライブラリへ登録 / プロジェクトへ配置

./my-skill を自分の Skill ディレクトリに置き換えてください。ディレクトリ名と SKILL.md の name は一致させます。

```sh
mskill add ./my-skill
mskill clone local/my-skill --project .
```

.agents/skills/my-skill に配置します。ライブラリの変更に追従する場合は link を選びます。Windows のリンクには開発者モードが必要な場合があります。

リンクに切り替える前に、同じ名前の既存インストールを対象プロジェクトから明示的に削除してください。管理対象外のディレクトリは上書きしません。

```sh
mskill remove my-skill --scope project --project .
mskill link local/my-skill --project .
```

## 取得と更新

owner/name をワークスペースに表示される完全な公開名に置き換えてください。公開パッケージの取得はログイン不要です。

```sh
mskill pull owner/name
mskill clone owner/name --project .
mskill update owner/name
```

## CLI から公開

```sh
mskill add ./my-skill
mskill login
mskill publish local/my-skill
```

再公開すると現在のパッケージを置換します。履歴が必要な場合は自分でバックアップしてください。

## 操作の範囲を守る

プロジェクト、ローカルライブラリ、クラウドの削除は別の操作です。ユーザーが明示的に求めた範囲だけを削除してください。クラウドの公開名はログイン中のアカウントに属する必要があります。プロジェクトのコピーは残りますが、ライブラリ削除はリンク切れを起こす場合があります。

```sh
mskill remove my-skill --scope project --project .
mskill remove local/my-skill --scope local
mskill remove my-owner-id/my-skill --scope cloud
```

## 知っておきたいこと

### ログインは必要？

ローカル管理と公開パッケージの取得には不要です。公開、クラウド管理、コミュニティへの参加には moeSegFault アカウントが必要です。

### 旧バージョンは残る？

残りません。公開名ごとに最新パッケージだけを保持し、update は SHA-256 が変わると置換します。プロジェクトのコピーは自動更新されず、リンクはライブラリに追従します。

### すぐに実行してよい？

まず内容を確認してください。取得やインストールは、同梱スクリプトの実行や指示に従うことへの許可ではありません。

- [Skill ワークスペースを開く](https://skills.moesegfault.dev/ja/)
- [ドキュメント](https://github.com/kleedaisuki/moesegfault-mskill#readme)
- [プライバシー](https://skills.moesegfault.dev/ja/privacy)
- [利用規約](https://skills.moesegfault.dev/ja/terms)
