# Preparar para GitHub

O checkout de desenvolvimento pode conter assets e informações privadas no histórico. **Não publique esse histórico por espelhamento nem adicione um remoto público para enviá-lo inteiro.** O .gitignore não apaga arquivos de commits anteriores.

## Gerar uma cópia selecionada

Execute na raiz do projeto de desenvolvimento, escolhendo uma pasta nova:

~~~powershell
python scripts/github/export_public.py E:/RagSWITCH-PreRenewal-GitHub --init-git
~~~

O comando recusa destinos existentes. Copia os arquivos atuais selecionados pela política, gera PUBLIC-SOURCE-MANIFEST.json, verifica o resultado e inicia uma branch main com histórico novo e identidade genérica de exportação. Preserva os avisos de autoria e os fontes de origem. O checkout de desenvolvimento e seus arquivos locais permanecem intactos.

A exportação não é uma sincronização automática: alterações posteriores precisam ser revisadas e uma nova exportação deve usar outro destino. O manifesto registra o commit de origem e que o estado atual da árvore de trabalho foi incluído; isso permite exportar mudanças ainda não commitadas sem fingir que pertencem ao commit indicado.

## Antes de tornar público

1. Resolva a questão de permissão descrita em [LICENSING.md](LICENSING.md). A seleção de arquivos e a CI não resolvem direitos de redistribuição.
2. Confira [STATUS.md](STATUS.md), licenças, avisos e conteúdo efetivamente selecionado. Revise também segredos e dados pessoais além dos padrões automáticos.
3. Valide a compilação em um checkout limpo com as receitas documentadas; o pacote atual ainda não passou por essa reconstrução completa.
4. Complete a importação dos overlays clássicos/PT-BR com dados fornecidos pelo jogador. O clone público não contém um pacote de jogo pronto.
5. Crie um repositório vazio no GitHub para o Pre-Renewal, envie somente a cópia revisada e habilite contribuições por forks e pull requests.

Nenhum comando deste procedimento cria repositório remoto nem envia arquivos ao GitHub.

## Releases

O NRO atual excede 100 MiB; não deve entrar no histórico Git comum. Após revisão do conteúdo redistribuível e validação, publique binários em Releases, acompanhados de notas, SHA256, fontes correspondentes e instruções de assets. Não anexe seu runtime inteiro com GRFs ou saves.

Comece como pre-release experimental. A campanha completa, a reconstrução em ambiente limpo e o desempenho no Switch ainda têm verificações pendentes.

Referências: [limites e binários grandes](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github), [licenças em repositórios](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository).
