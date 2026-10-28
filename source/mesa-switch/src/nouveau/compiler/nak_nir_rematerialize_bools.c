/*
 * SPDX-License-Identifier: MIT
 */

/*
 * Re-materialize cheap booleans in the blocks that use them.  The block walk
 * follows nak_nir_rematerialize_load_const.c.
 *
 * A switch is lowered to one "sel == k" compare per case.  CSE leaves each
 * compare in the block of the switch, where the default test consumes it,
 * and the case branches then keep every compare live until they are reached.
 * Maxwell has seven predicates, so the rest are spilled to general registers.
 * Cloning the compare into each using block keeps only the selector live.
 *
 * Only compares against a constant are cloned, possibly behind a shallow
 * tree of iand/ior/ixor/inot, so a clone extends at most one value per
 * leaf.  The results are bit-identical.  No CSE may run after this pass.
 */

#include "nak_private.h"
#include "nir_builder.h"
#include "util/hash_table.h"

#define REMAT_MAX_DEPTH 3

struct remat_ctx {
   struct hash_table *remap;
   nir_block *block;
   nir_builder b;
   bool progress;
};

static bool
is_bool_logic(nir_op op)
{
   return op == nir_op_iand || op == nir_op_ior ||
          op == nir_op_ixor || op == nir_op_inot;
}

static bool
is_remat_bool(nir_def *def, unsigned depth)
{
   nir_instr *instr = nir_def_instr(def);
   if (instr->type == nir_instr_type_load_const)
      return true;

   if (instr->type != nir_instr_type_alu ||
       def->bit_size != 1 || def->num_components != 1)
      return false;

   nir_alu_instr *alu = nir_instr_as_alu(instr);
   const unsigned num_srcs = nir_op_infos[alu->op].num_inputs;

   if (is_bool_logic(alu->op)) {
      if (depth == 0)
         return false;

      for (unsigned i = 0; i < num_srcs; i++) {
         if (!is_remat_bool(alu->src[i].src.ssa, depth - 1))
            return false;
      }
      return true;
   }

   if (!nir_alu_instr_is_comparison(alu))
      return false;

   for (unsigned i = 0; i < num_srcs; i++) {
      if (nir_src_is_const(alu->src[i].src))
         return true;
   }
   return false;
}

static nir_def *
remat_clone(struct remat_ctx *ctx, nir_def *def)
{
   nir_instr *instr = nir_def_instr(def);
   if (instr->type == nir_instr_type_load_const)
      return def;

   struct hash_entry *entry = _mesa_hash_table_search(ctx->remap, def);
   if (entry != NULL)
      return entry->data;

   nir_alu_instr *alu = nir_instr_as_alu(instr);
   nir_def *srcs[NIR_ALU_MAX_INPUTS] = { NULL };
   const unsigned num_srcs = nir_op_infos[alu->op].num_inputs;

   if (is_bool_logic(alu->op)) {
      for (unsigned i = 0; i < num_srcs; i++)
         srcs[i] = remat_clone(ctx, alu->src[i].src.ssa);
   }

   nir_alu_instr *clone = nir_alu_instr_clone(ctx->b.shader, alu);
   nir_builder_instr_insert(&ctx->b, &clone->instr);

   for (unsigned i = 0; i < num_srcs; i++) {
      if (srcs[i] != NULL)
         nir_src_rewrite(&clone->src[i].src, srcs[i]);
   }

   _mesa_hash_table_insert(ctx->remap, def, &clone->def);
   return &clone->def;
}

static bool
remat_src(nir_src *src, void *_ctx)
{
   struct remat_ctx *ctx = _ctx;
   nir_def *def = src->ssa;

   if (def->bit_size != 1 || nir_def_block(def) == ctx->block)
      return true;

   if (nir_def_instr(def)->type != nir_instr_type_alu ||
       !is_remat_bool(def, REMAT_MAX_DEPTH))
      return true;

   nir_src_rewrite(src, remat_clone(ctx, def));
   ctx->progress = true;
   return true;
}

static bool
rematerialize_bools_impl(nir_function_impl *impl)
{
   struct remat_ctx ctx = {
      .remap = _mesa_pointer_hash_table_create(NULL),
      .b = nir_builder_create(impl),
   };

   nir_foreach_block(block, impl) {
      _mesa_hash_table_clear(ctx.remap, NULL);
      ctx.block = block;

      nir_foreach_instr(instr, block) {
         if (instr->type == nir_instr_type_phi)
            continue;

         ctx.b.cursor = nir_before_instr(instr);
         nir_foreach_src(instr, remat_src, &ctx);
      }

      nir_if *nif = nir_block_get_following_if(block);
      if (nif != NULL) {
         ctx.b.cursor = nir_after_block_before_jump(block);
         remat_src(&nif->condition, &ctx);
      }
   }

   _mesa_hash_table_destroy(ctx.remap, NULL);

   return nir_progress(ctx.progress, impl, nir_metadata_control_flow);
}

bool
nak_nir_rematerialize_bools(nir_shader *nir)
{
   bool progress = false;

   nir_foreach_function_impl(impl, nir)
      progress |= rematerialize_bools_impl(impl);

   return progress;
}
