# EasyList / EasyPrivacy domain filter data

Source: **The EasyList authors — https://easylist.to/**.

This bundled hostname dataset is adapted from the EasyList repository and distributed under **Creative Commons Attribution-ShareAlike 3.0 Unported, or a later version**, as permitted by the upstream dual licensing notice: https://easylist.to/pages/licence.html.

License terms: https://creativecommons.org/licenses/by-sa/3.0/legalcode. Attribution and this license apply to the filter data and adaptations of that data.

Upstream inputs:

- https://github.com/easylist/easylist/blob/master/easylist/easylist_adservers.txt
- https://github.com/easylist/easylist/blob/master/easyprivacy/easyprivacy_trackingservers_general.txt

Changes: only unconditional `||hostname^` rules are extracted. Conditional rules, path rules, wildcards, exceptions, and cosmetic selectors are not imported. Hostnames are normalized to lowercase, sorted, and deduplicated. Seven explicit Azy Core additions cover common Google advertising/analytics domains; those additions are listed in `snapshot.json`, together with the exact upstream revision and source hashes. This is a domain blocker, not a full implementation of EasyList filter syntax, and it is not endorsed by EasyList.
