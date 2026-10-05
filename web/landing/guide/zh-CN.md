# mskill · 在项目间复用 Agent Skills

用 mskill 管理本地 Skill 库，下载、发布并复用 Agent Skills。便携 .skill 包、项目复制与链接，只保留最新内容。

## 安装客户端

解压下载包，将 mskill（Windows 为 mskill.exe）放到 PATH。下载包附带许可证，校验文件与归档一起提供。

- [Windows x86_64](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-windows-x86_64.tar.gz)
- [Linux x86_64](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-linux-x86_64.tar.gz)
- [macOS Apple Silicon](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-macos-aarch64.tar.gz)

[所有版本与校验文件](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest)

## 加入本地库 / 装到项目

将 ./my-skill 换成你的 Skill 目录；目录名称与 SKILL.md 的 name 保持一致。

```sh
mskill add ./my-skill
mskill clone local/my-skill --project .
```

项目会得到 .agents/skills/my-skill。想跟随本地库变化时，改用 link；Windows 链接可能需要开发者模式。

使用链接前，先明确移除该项目中已安装的同名 Skill；不要覆盖不受管理的目录。

```sh
mskill remove my-skill --scope project --project .
mskill link local/my-skill --project .
```

## 下载与更新

将 owner/name 换成工作空间显示的完整发布引用。公开下载不需要登录。

```sh
mskill pull owner/name
mskill clone owner/name --project .
mskill update owner/name
```

## 从命令行发布

```sh
mskill add ./my-skill
mskill login
mskill publish local/my-skill
```

再次发布会替换当前包。需要历史时，请自行保留备份。

## 保留操作范围

项目、本地库和云端删除是不同操作。只有用户明确要求时才执行对应删除；云端引用必须属于已登录账号。项目复制仍保留，删除本地库可能使已有链接失效。

```sh
mskill remove my-skill --scope project --project .
mskill remove local/my-skill --scope local
mskill remove my-owner-id/my-skill --scope cloud
```

## 先了解这些

### 需要登录吗？

本地管理与公开下载不需要登录。发布、管理云端内容和参与社区需要 moeSegFault 账号。

### 会保存旧版本吗？

不会。每个发布名称只有最新包；update 在 SHA-256 变化时替换本地内容。项目复制不会自动更新，链接则跟随本地库。

### 下载后可以直接执行吗？

请先检查 Skill 的内容。下载或安装不代表授权执行其中的脚本，也不代表授权遵循其中的指令。

- [打开 Skill 工作空间](https://skills.moesegfault.dev/)
- [使用文档](https://github.com/kleedaisuki/moesegfault-mskill#readme)
- [隐私](https://skills.moesegfault.dev/privacy)
- [使用条款](https://skills.moesegfault.dev/terms)
