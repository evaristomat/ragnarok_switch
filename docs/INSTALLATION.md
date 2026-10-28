# Instalar no Nintendo Switch

Esta edição é experimental. Faça backup da pasta do jogo no SD antes de atualizar, principalmente do banco e da configuração.

## Estrutura no SD

~~~text
/switch/ragswitch-pre/
  ragswitch-pre.nro
  game-data/
    data.grf
    official_data.grf
    ragswitch-assets.grf
    ...demais dados locais configurados
~~~

O pacote local completo preparado pelo mantenedor está em runtime/switch/ragswitch-pre. Essa pasta não é incluída no Git público. Quem compila deve preparar seus próprios arquivos, conforme [ASSETS.md](ASSETS.md).

Copie o NRO para a pasta da edição e os arquivos locais para game-data. Abra pelo hbmenu em modo aplicativo: segure R enquanto inicia um jogo instalado e abra o NRO. O modo applet pode não oferecer memória suficiente.

O executável fixa seus arquivos em sdmc:/switch/ragswitch-pre. Na primeira inicialização, extrai o servidor e cria o save. Alterar o nome da pasta do NRO não cria automaticamente uma edição com save separado; um fork que deseje outra identidade precisa revisar os caminhos e o identificador da edição nos fontes.

## Atualizar

Feche o jogo antes de copiar. Preserve ragnarok.db, seus arquivos auxiliares quando presentes e a configuração existente. Troque somente o NRO e os recursos exigidos pela versão. Não copie o SQL inicial por cima de um save; ele existe para gerar a base embarcada, não para migrar personagens.

Não misture banco ou configuração da Renewal. O servidor verifica a identidade do banco e recusa uma edição incompatível.

Os instaladores DBI antigos são ferramentas do ambiente do mantenedor, não fazem parte do fluxo público. A cópia manual pelo DBI/MTP ou leitor de SD deve ser feita com backup e o jogo fechado.

## Reportar problemas

Informe a versão, mapa, ação que causou o erro, modo de abertura e se há bots/companheiros ativos. Consulte [TESTING.md](TESTING.md) antes de compartilhar logs ou bancos pessoais.
