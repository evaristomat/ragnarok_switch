# Instalar no Nintendo Switch

Esta edição é experimental. Faça backup da pasta do jogo no SD antes de atualizar, principalmente do banco e da configuração.

## O que baixar antes de começar

1. **Executável:** na [release 0.1.0-pre12](https://github.com/evaristomat/ragnarok_switch/releases/tag/v0.1.0-pre12), baixe `ragswitch-pre.nro` e, para conferir a transferência, `SHA256SUMS.txt`. O arquivo **Source code** é para desenvolvimento e não contém o executável. O NRO tem 149.062.785 bytes, aproximadamente 149 MB.
2. **Cliente compatível:** utilize os arquivos do kRO **2025-07-16**. As instruções de obtenção do cliente estão no projeto de origem [WARP0716, Getting the Client](https://github.com/CrazyBebop/WARP0716#getting-the-client), que descreve o WARPGATE. Preserve `data.grf`, `System/`, `SystemEN/` e `data/`, quando presentes. Os executáveis Windows `.exe`/`.dll` não são necessários no Switch.
3. **Recursos adicionais:** siga as instruções de [custom officials do ROenglishRE](https://llchrisll.github.io/ROTPDocs/misc/custom_officials/) para obter `official_data.grf`. A página é mantida pelo projeto de origem; versões e disponibilidade podem mudar.
4. **Overlay desta edição:** baixe [ragswitch-assets.grf](https://github.com/evaristomat/ragnarok_switch/releases/download/v0.1.0-pre12/ragswitch-assets.grf) na mesma release. Ele contém as substituições clássicas e os ajustes PT-BR, tem 21.016.768 bytes (aproximadamente 21 MB) e deve ficar em `/switch/ragswitch-pre/game-data/ragswitch-assets.grf`. Não é necessário gerar esse arquivo para instalar a pre12. Leia [ASSETS.md](ASSETS.md).
5. **Música opcional:** copie a pasta `BGM/` do cliente compatível se quiser música.

Tenha um Nintendo Switch já configurado para executar homebrew pelo [hbmenu](https://github.com/switchbrew/nx-hbmenu), um microSD e um PC/leitor de cartão ou transferência DBI/MTP. Este guia começa com o ambiente homebrew funcionando; não configura o desbloqueio do console. Reserve pelo menos 6 GB para os dados base, mais espaço para música, servidor extraído e saves. Os três GRFs locais somam aproximadamente 4,46 GB; os tamanhos podem variar conforme o pacote de origem.

Não é necessário compilar o projeto, instalar o APK Android, executar servidores no PC, importar um banco manualmente ou usar uma conta de Ragnarok online. O servidor local e o banco inicial sem personagens estão embarcados no NRO. Os recursos externos precisam ser obtidos e usados com as permissões aplicáveis.

## Estrutura no SD

~~~text
/switch/ragswitch-pre/
  ragswitch-pre.nro
  game-data/
    data.grf
    official_data.grf
    ragswitch-assets.grf
    System/
    SystemEN/
    data/
    BGM/                  opcional
~~~

O pacote local completo preparado pelo mantenedor não é incluído no Git público. A release fornece o NRO e `ragswitch-assets.grf`; obtenha os demais recursos conforme [ASSETS.md](ASSETS.md). A pasta `ragswitch-pre` deve ficar diretamente dentro de `switch`, sem uma segunda pasta intermediária criada pela extração de um ZIP.

## Copiar e abrir

1. Feche o jogo e faça backup se já tiver esta edição instalada.
2. Copie o NRO para a pasta da edição e os arquivos locais para `game-data`, como na árvore acima. Preserve os nomes dos arquivos.
3. Abra o hbmenu em **modo aplicativo**: segure **R** enquanto inicia um jogo instalado, mantenha o botão durante a abertura e selecione RagSWITCH Pre-Renewal no hbmenu. O modo applet pelo Álbum pode não oferecer memória suficiente.
4. Aguarde a preparação inicial. O servidor é extraído do executável e o save é criado no cartão. Não interrompa essa primeira preparação.
5. Prossiga pela interface offline e crie seu personagem. Não há servidor externo para configurar ou pagamento/conta online exigidos por este aplicativo.

O executável fixa seus arquivos em sdmc:/switch/ragswitch-pre. Na primeira inicialização, extrai o servidor e cria o save. Alterar o nome da pasta do NRO não cria automaticamente uma edição com save separado; um fork que deseje outra identidade precisa revisar os caminhos e o identificador da edição nos fontes.

## Conferir o download

No PowerShell, dentro da pasta em que baixou o NRO:

~~~powershell
Get-FileHash .\ragswitch-pre.nro -Algorithm SHA256
Get-FileHash .\ragswitch-assets.grf -Algorithm SHA256
~~~

Compare com `SHA256SUMS.txt` da release. Para esta pre12, o SHA256 esperado é:

~~~text
d496a0461e94326f07edb9d524982fb4bcf7058ed527dc2b33d77724ebed1c22
~~~

Para `ragswitch-assets.grf`, o SHA256 esperado é:

~~~text
5fa11aa9056afed4df523264aae86d765a1b6d4b6388ab8a80d79ac964a1619a
~~~

Se houver diferença, baixe ou copie novamente antes de substituir um executável funcional.

## Atualizar

Feche o jogo antes de copiar. Preserve ragnarok.db, seus arquivos auxiliares quando presentes e a configuração existente. Troque somente o NRO e os recursos exigidos pela versão. Não copie o SQL inicial por cima de um save; ele existe para gerar a base embarcada, não para migrar personagens.

Não misture banco ou configuração da Renewal. O servidor verifica a identidade do banco e recusa uma edição incompatível.

Os instaladores DBI antigos são ferramentas do ambiente do mantenedor, não fazem parte do fluxo público. A cópia manual pelo DBI/MTP ou leitor de SD deve ser feita com backup e o jogo fechado.

## Problemas frequentes

| Sintoma | O que conferir |
| --- | --- |
| `Arquivo de jogo ausente` | Confira o caminho exibido, os três GRFs e a pasta fixa. Baixe o overlay na release e copie-o dentro de `game-data/`, não ao lado do NRO. |
| NRO não aparece no hbmenu | Confira `/switch/ragswitch-pre/ragswitch-pre.nro` e se o arquivo baixado é o NRO da release. |
| Falta de memória ou falha ao abrir pelo Álbum | Abra em modo aplicativo, segurando R ao iniciar um jogo instalado. |
| Personagens/monstros invisíveis ou recursos faltando | Confira `official_data.grf`, a versão do cliente e os demais recursos compatíveis. |
| Paredes, terreno ou posições não correspondem ao servidor | Confira o overlay de mapas clássicos; não substitua por um GRF moderno arbitrário. |
| Sem música | Confira os arquivos em `game-data/BGM/`. A música é opcional. |
| Travamento mesmo com os arquivos certos | A versão é experimental. Registre versão, mapa e ação e consulte [TESTING.md](TESTING.md). Não apague seu banco para tentar corrigir. |

## Reportar problemas

Informe a versão, mapa, ação que causou o erro, modo de abertura e se há bots/companheiros ativos. Consulte [TESTING.md](TESTING.md) antes de compartilhar logs ou bancos pessoais.
